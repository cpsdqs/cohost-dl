use crate::post::PostBlock;
use deno_core::_ops::RustToV8;
use deno_core::{ascii_str, ascii_str_include, serde_v8, v8, JsRuntime};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use tokio::sync::oneshot;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostRenderRequest {
    pub blocks: Vec<PostBlock>,
    pub published_at: String,
    pub has_cohost_plus: bool,
    pub disable_embeds: bool,
    pub external_links_in_new_tab: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostRenderResult {
    html: String,
}

struct QueueItem {
    req: PostRenderRequest,
    ret: oneshot::Sender<anyhow::Result<PostRenderResult>>,
}

pub struct PostRenderer {
    queue: Arc<Mutex<VecDeque<QueueItem>>>,
    signal: Arc<Condvar>,
}

impl PostRenderer {
    pub fn new(renderers: usize) -> Self {
        // is there a better solution to this? I am not going to find out right now
        let queue = Arc::new(Mutex::new(VecDeque::<QueueItem>::new()));
        let signal = Arc::new(Condvar::new());

        for i in 0..renderers {
            let queue = Arc::clone(&queue);
            let signal = Arc::clone(&signal);
            let _ = std::thread::Builder::new()
                .name(format!("post render {i}"))
                .spawn(|| {
                    let renderer = ThreadPostRenderer::new();

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

                            let result = renderer.render_post(item.req).await;
                            let _ = item.ret.send(result);
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
            queue.push_back(QueueItem { req, ret });
            self.signal.notify_one();
        }

        recv.await?
    }
}

struct ThreadPostRenderer {
    rt: RefCell<JsRuntime>,
    render_post_fn: v8::Global<v8::Function>,
}

impl ThreadPostRenderer {
    fn new() -> Self {
        let mut rt = JsRuntime::new(Default::default());
        rt.execute_script(
            "render.js",
            ascii_str_include!("../post-render/compiled.js"),
        )
        .expect("post render script error");

        let render_post_fn = {
            let main_context = rt.main_context();
            let main_context2 = rt.main_context();

            let mut scope = v8::HandleScope::with_context(rt.v8_isolate(), main_context);
            let global_ctx = v8::Local::new(&mut scope, main_context2);
            let global = global_ctx.global(&mut scope);

            let render_post_name = ascii_str!("renderPost").v8_string(&mut scope);
            let render_post_fn = global
                .get(&mut scope, render_post_name.into())
                .expect("missing renderPost global");
            let render_post_fn = v8::Local::<v8::Function>::try_from(render_post_fn)
                .expect("renderPost is not a function");

            v8::Global::new(&mut scope, render_post_fn)
        };

        Self {
            rt: RefCell::new(rt),
            render_post_fn,
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
        let event_loop = rt.run_event_loop(Default::default());

        let (result, event_loop) = tokio::join! {
            result,
            event_loop,
        };

        event_loop?;

        let main_context = rt.main_context();
        let mut scope = v8::HandleScope::with_context(rt.v8_isolate(), main_context);
        let result = result?.to_v8(&mut scope);
        let result = serde_v8::from_v8(&mut scope, result)?;

        Ok(result)
    }
}