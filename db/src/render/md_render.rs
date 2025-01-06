use crate::bundled_files::MD_RENDER_COMPILED;
use crate::post::PostBlock;
use deno_core::_ops::RustToV8;
use deno_core::url::Url;
use deno_core::{ascii_str, serde_v8, v8, JsRuntime, RuntimeOptions};
use deno_web::TimersPermission;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use tokio::sync::oneshot;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostRenderRequest {
    pub post_id: u64,
    pub blocks: Vec<PostBlock>,
    pub published_at: String,
    pub has_cohost_plus: bool,
    pub resources: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostRenderResult {
    pub preview: String,
    pub full: Option<String>,
    pub class_name: String,
    pub view_model: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownRenderRequest {
    pub markdown: String,
    pub published_at: String,
    pub context: MarkdownRenderContext,
    pub has_cohost_plus: bool,
    pub resources: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkdownRenderContext {
    Profile,
    Comment,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkdownRenderResult {
    html: String,
}

enum QueueItem {
    Post {
        req: PostRenderRequest,
        ret: oneshot::Sender<anyhow::Result<PostRenderResult>>,
    },
    Markdown {
        req: MarkdownRenderRequest,
        ret: oneshot::Sender<anyhow::Result<MarkdownRenderResult>>,
    },
}

pub struct MarkdownRenderer {
    queue: Arc<Mutex<VecDeque<QueueItem>>>,
    signal: Arc<Condvar>,
}

impl MarkdownRenderer {
    pub fn new(renderers: usize) -> Self {
        JsRuntime::init_platform(None, true);

        // is there a better solution to this? I am not going to find out right now
        let queue = Arc::new(Mutex::new(VecDeque::<QueueItem>::new()));
        let signal = Arc::new(Condvar::new());

        for i in 0..renderers {
            let queue = Arc::clone(&queue);
            let signal = Arc::clone(&signal);
            let _ = std::thread::Builder::new()
                .name(format!("post render {i}"))
                .spawn(|| {
                    let renderer = ThreadMarkdownRenderer::new();

                    let rt = tokio::runtime::Builder::new_current_thread()
                        .build()
                        .unwrap();

                    let local_set = tokio::task::LocalSet::new();
                    let fut = local_set.run_until(async move {
                        loop {
                            let item = loop {
                                let mut queue = queue.lock().unwrap();
                                if let Some(item) = queue.pop_front() {
                                    break item;
                                }
                                while queue.is_empty() {
                                    queue = signal.wait(queue).unwrap();
                                }
                            };

                            match item {
                                QueueItem::Post { req, ret } => {
                                    let result = renderer.render_post(req).await;
                                    let _ = ret.send(result);
                                }
                                QueueItem::Markdown { req, ret } => {
                                    let result = renderer.render_markdown(req).await;
                                    let _ = ret.send(result);
                                }
                            }
                        }
                    });

                    rt.block_on(fut);
                });
        }

        Self { queue, signal }
    }

    pub async fn render_post(&self, req: PostRenderRequest) -> anyhow::Result<PostRenderResult> {
        let (ret, recv) = oneshot::channel();

        {
            let mut queue = self.queue.lock().unwrap();
            queue.push_back(QueueItem::Post { req, ret });
            self.signal.notify_one();
        }

        recv.await?
    }

    pub async fn render_markdown(
        &self,
        req: MarkdownRenderRequest,
    ) -> anyhow::Result<MarkdownRenderResult> {
        let (ret, recv) = oneshot::channel();

        {
            let mut queue = self.queue.lock().unwrap();
            queue.push_back(QueueItem::Markdown { req, ret });
            self.signal.notify_one();
        }

        recv.await?
    }
}

struct ThreadMarkdownRenderer {
    rt: RefCell<JsRuntime>,
    render_post_fn: v8::Global<v8::Function>,
    render_markdown_fn: v8::Global<v8::Function>,
}

deno_core::extension!(
    small_runtime,
    esm_entry_point = "ext:small_runtime/md_render_rt.js",
    esm = [dir "src/render", "md_render_rt.js"],
);

struct AllowHrTime;

impl TimersPermission for AllowHrTime {
    fn allow_hrtime(&mut self) -> bool {
        true
    }
}

impl ThreadMarkdownRenderer {
    fn new() -> Self {
        let mut rt = JsRuntime::new(RuntimeOptions {
            extensions: vec![
                deno_webidl::deno_webidl::init_ops_and_esm(),
                deno_console::deno_console::init_ops_and_esm(),
                deno_url::deno_url::init_ops_and_esm(),
                deno_web::deno_web::init_ops_and_esm::<AllowHrTime>(
                    Arc::new(Default::default()),
                    Some(Url::parse("https://cohost.org/").unwrap()),
                ),
                small_runtime::init_ops_and_esm(),
            ],
            ..Default::default()
        });

        let render_module = rt
            .lazy_load_es_module_with_code("file:///render.js", MD_RENDER_COMPILED)
            .expect("md render script error");

        let (render_post_fn, render_markdown_fn) = {
            let mut scope = rt.handle_scope();

            let exports = v8::Local::new(&mut scope, render_module);
            let exports = v8::Local::<v8::Object>::try_from(exports).expect("no exports");

            let render_post_name = ascii_str!("renderPost").v8_string(&mut scope);
            let render_post_fn = exports
                .get(&mut scope, render_post_name.into())
                .expect("missing renderPost export");
            let render_post_fn = v8::Local::<v8::Function>::try_from(render_post_fn)
                .expect("renderPost is not a function");

            let render_post_fn = v8::Global::new(&mut scope, render_post_fn);

            let render_markdown_name = ascii_str!("renderMarkdown").v8_string(&mut scope);
            let render_markdown_fn = exports
                .get(&mut scope, render_markdown_name.into())
                .expect("missing renderMarkdown export");
            let render_markdown_fn = v8::Local::<v8::Function>::try_from(render_markdown_fn)
                .expect("renderMarkdown is not a function");

            let render_markdown_fn = v8::Global::new(&mut scope, render_markdown_fn);

            (render_post_fn, render_markdown_fn)
        };

        Self {
            rt: RefCell::new(rt),
            render_post_fn,
            render_markdown_fn,
        }
    }

    async fn render_post(&self, options: PostRenderRequest) -> anyhow::Result<PostRenderResult> {
        let mut rt = self.rt.borrow_mut();

        let options = {
            let main_context = rt.main_context();
            let mut scope = v8::HandleScope::with_context(rt.v8_isolate(), main_context);
            let options = serde_v8::to_v8(&mut scope, options)?;
            v8::Global::new(&mut scope, options)
        };

        let result = rt.call_with_args(&self.render_post_fn, &[options]);
        let result = rt
            .with_event_loop_promise(result, Default::default())
            .await?;

        let main_context = rt.main_context();
        let mut scope = v8::HandleScope::with_context(rt.v8_isolate(), main_context);
        let result = result.to_v8(&mut scope);
        let result = serde_v8::from_v8(&mut scope, result)?;

        Ok(result)
    }

    async fn render_markdown(
        &self,
        options: MarkdownRenderRequest,
    ) -> anyhow::Result<MarkdownRenderResult> {
        let mut rt = self.rt.borrow_mut();

        let options = {
            let main_context = rt.main_context();
            let mut scope = v8::HandleScope::with_context(rt.v8_isolate(), main_context);
            let options = serde_v8::to_v8(&mut scope, options)?;
            v8::Global::new(&mut scope, options)
        };

        let result = rt.call_with_args(&self.render_markdown_fn, &[options]);
        let result = rt
            .with_event_loop_promise(result, Default::default())
            .await?;

        let main_context = rt.main_context();
        let mut scope = v8::HandleScope::with_context(rt.v8_isolate(), main_context);
        let result = result.to_v8(&mut scope);
        let result = serde_v8::from_v8(&mut scope, result)?;

        Ok(result)
    }
}
