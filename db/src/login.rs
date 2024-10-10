use crate::context::USER_AGENT;
use anyhow::{bail, Context};
use base64::prelude::{BASE64_STANDARD, BASE64_STANDARD_NO_PAD};
use base64::Engine;
use pbkdf2::hmac::Hmac;
use pbkdf2::pbkdf2;
use reqwest::{Client, Method, Url};
use serde::{Deserialize, Serialize};
use sha2::Sha384;

async fn trpc<I, R>(
    client: &Client,
    cookie: &str,
    method: Method,
    id: &str,
    input: I,
) -> anyhow::Result<R>
where
    I: Serialize,
    R: for<'a> Deserialize<'a>,
{
    let mut url = Url::parse(&format!("https://cohost.org/api/v1/trpc/{id}"))?;

    if method == Method::GET {
        url.query_pairs_mut()
            .append_pair("input", &serde_json::to_string(&input)?);
    }

    let mut req = client.request(method.clone(), url).header("cookie", cookie);

    if method == Method::POST {
        req = req
            .header("content-type", "application/json")
            .body(serde_json::to_string(&input)?);
    }

    let res = req.send().await?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await?;
        bail!("unexpected: {status}\n{text}");
    }

    #[derive(Deserialize)]
    enum TrpcResult<T> {
        #[serde(rename = "result")]
        Result { data: T },
        #[serde(rename = "error")]
        Error {
            code: i64,
            message: String,
        },
    }

    let result = res.text().await?;
    match serde_json::from_str::<TrpcResult<R>>(&result)? {
        TrpcResult::Result { data } => Ok(data),
        TrpcResult::Error { code, message, .. } => {
            bail!("{code}: {message}");
        }
    }
}

pub async fn login(email: &str, password: &str) -> anyhow::Result<(String, bool)> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("failed to create client");

    let cookie = {
        let login_page = client.get("https://cohost.org/rc/login").send().await?;

        if !login_page.status().is_success() {
            bail!("could not get login page");
        }
        let cookie_header = login_page
            .headers()
            .get("set-cookie")
            .and_then(|cookie| cookie.to_str().ok().map(|s| s.to_string()));

        let cookie = if let Some(cookie_header) = cookie_header {
            let Some(cookie) = cookie_header.split(';').next() else {
                bail!("bad cookie header");
            };
            cookie.to_string()
        } else {
            bail!("no set-cookie header");
        };

        cookie
    };

    let salt = {
        #[derive(Serialize)]
        struct GetSalt {
            email: String,
        }
        #[derive(Deserialize)]
        struct Salt {
            salt: String,
        }
        let salt: Salt = trpc(
            &client,
            &cookie,
            Method::GET,
            "login.getSalt",
            GetSalt {
                email: email.to_string(),
            },
        )
        .await
        .context("getting salt")?;

        BASE64_STANDARD_NO_PAD.decode(salt.salt).context("decoding salt")?
    };

    let hash = {
        let mut result = [0; 128];
        pbkdf2::<Hmac<Sha384>>(password.as_bytes(), &salt, 200_000, &mut result)?;
        BASE64_STANDARD.encode(result)
    };

    let needs_otp = {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Login {
            client_hash: String,
            email: String,
        }

        #[derive(PartialEq, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        enum State {
            NeedOtp,
            Done,
        }

        #[derive(Deserialize)]
        struct LoginResponse {
            state: State,
        }

        let res: LoginResponse = trpc(
            &client,
            &cookie,
            Method::POST,
            "login.login",
            Login {
                client_hash: hash,
                email: email.to_string(),
            },
        )
        .await
        .context("logging in")?;

        res.state == State::NeedOtp
    };

    Ok((cookie, needs_otp))
}

pub async fn login_otp(cookie: &str, otp: &str) -> anyhow::Result<()> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("failed to create client");

    #[derive(Serialize)]
    struct Req {
        token: String,
    }

    #[derive(Deserialize)]
    struct Res {
        reset: bool,
    }

    let res: Res = trpc(
        &client,
        &cookie,
        Method::POST,
        "login.send2FAToken",
        Req {
            token: otp.to_string(),
        },
    )
    .await
    .context("error in 2FA")?;

    if res.reset {
        bail!("unexpected response: reset");
    }

    Ok(())
}
