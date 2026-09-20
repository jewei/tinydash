//! Application images are independent of search and have bounded retained work.
use base64::Engine;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};
use tokio::sync::oneshot;

use crate::providers::apps::AppEntry;

const CACHE_BYTES: usize = 2 * 1024 * 1024;
const CACHE_ENTRIES: usize = 256;
const ACTIVE_LOADS: usize = 2;
const QUEUED_LOADS: usize = 64;
const WAITERS: usize = 128;
type Image = Arc<Vec<u8>>;
type Loader = dyn Fn(&Path, u16) -> Option<Vec<u8>> + Send + Sync;
type Notify = dyn Fn() + Send + Sync;
type Reply = Result<Image, IconError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IconError {
    Unavailable,
    InvalidSize,
    Busy,
    Cancelled,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Key {
    source: String,
    pixels: u16,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Request {
    window: String,
    id: String,
}
struct Job {
    active: bool,
    path: PathBuf,
    replies: HashMap<Request, oneshot::Sender<Reply>>,
}
struct Cached {
    value: Reply,
    used: u64,
}
impl Cached {
    fn bytes(&self) -> usize {
        self.value.as_ref().map_or(0, |bytes| bytes.len())
    }
}

#[derive(Default)]
struct State {
    generation: u64,
    catalog: HashMap<String, PathBuf>,
    cache: HashMap<Key, Cached>,
    bytes: usize,
    clock: u64,
    jobs: HashMap<Key, Job>,
    queue: VecDeque<Key>,
    active: usize,
    blocked_waiters: bool,
    blocked_queue: bool,
}

impl State {
    fn waiter_count(&self) -> usize {
        self.jobs.values().map(|job| job.replies.len()).sum()
    }

    // A broadcast is useful only after a rejected request can retry. Keep
    // separate reasons: a duplicate may join a job even while the queue is full.
    fn take_capacity_notification(&mut self) -> bool {
        let waiter_slot = self.waiter_count() < WAITERS;
        let waiters = self.blocked_waiters && waiter_slot;
        let queue = self.blocked_queue && waiter_slot && self.queue.len() < QUEUED_LOADS;
        self.blocked_waiters &= !waiters;
        self.blocked_queue &= !queue;
        waiters || queue
    }
}

struct Inner {
    state: Mutex<State>,
    loader: Arc<Loader>,
    budget: usize,
    enabled: bool,
}

#[derive(Clone)]
pub struct IconStore(Arc<Inner>);
impl Default for IconStore {
    fn default() -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(State::default()),
            loader: Arc::new(|path, pixels| {
                crate::platform::application_icon(path, pixels).map(|bytes| {
                    format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    )
                    .into_bytes()
                })
            }),
            budget: CACHE_BYTES,
            enabled: cfg!(target_os = "macos"),
        }))
    }
}

// Finder can use an .icns file, an asset catalog, or a custom Icon\r resource.
// A catalog refresh creates new identities even if the OS timestamp resolution
// cannot distinguish two replacements.
fn source_revision(path: &Path) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let resources = path.join("Contents/Resources");
    let mut files = vec![
        path.to_owned(),
        path.join("Icon\r"),
        path.join("Contents/Info.plist"),
        resources.clone(),
    ];
    if let Ok(entries) = std::fs::read_dir(resources) {
        files.extend(entries.flatten().map(|entry| entry.path()).filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "icns" || extension == "car")
        }));
    }
    files.sort();
    for file in files {
        file.hash(&mut hash);
        if let Ok(meta) = file.metadata() {
            meta.len().hash(&mut hash);
            meta.modified().ok().hash(&mut hash);
        }
    }
    hash.finish()
}

impl IconStore {
    async fn image(
        &self,
        source: String,
        pixels: u16,
        request: Request,
        ready: Arc<Notify>,
    ) -> Result<String, IconError> {
        let receiver = self.request(source, pixels, request, ready)?;
        let result = receiver.await.map_err(|_| IconError::Cancelled)?;
        let bytes = result?;
        String::from_utf8(bytes.as_ref().clone()).map_err(|_| IconError::Unavailable)
    }

    fn cancel_request(&self, request: &Request, ready: impl Fn()) {
        if self.cancel(request) {
            ready();
        }
    }

    pub fn replace_catalog(&self, entries: &mut [AppEntry], ready: impl Fn()) {
        let sources: Vec<_> = if self.0.enabled {
            entries
                .iter()
                .map(|entry| (entry.path.clone(), source_revision(&entry.path)))
                .collect()
        } else {
            Vec::new()
        };
        let mut state = self.0.state.lock().unwrap();
        state.generation += 1;
        state.catalog.clear();
        state.cache.clear();
        state.bytes = 0;
        state.queue.clear();
        state.jobs.retain(|_, job| {
            for (_, reply) in job.replies.drain() {
                let _ = reply.send(Err(IconError::Cancelled));
            }
            job.active
        });
        for (index, entry) in entries.iter_mut().enumerate() {
            entry.icon = sources.get(index).map(|(path, revision)| {
                let key = format!("app-icon:{}:{index}:{revision:x}", state.generation);
                state.catalog.insert(key.clone(), path.clone());
                key
            });
        }
        let notify = state.take_capacity_notification();
        drop(state);
        if notify {
            ready();
        }
    }

    fn request(
        &self,
        source: String,
        pixels: u16,
        request: Request,
        ready: Arc<Notify>,
    ) -> Result<oneshot::Receiver<Reply>, IconError> {
        if !(16..=256).contains(&pixels) {
            return Err(IconError::InvalidSize);
        }
        if request.id.len() > 96 {
            return Err(IconError::Unavailable);
        }
        let key = Key { source, pixels };
        let (reply, receiver) = oneshot::channel();
        {
            let mut state = self.0.state.lock().unwrap();
            let path = state
                .catalog
                .get(&key.source)
                .ok_or(IconError::Unavailable)?
                .clone();
            state.clock += 1;
            let now = state.clock;
            if let Some(cached) = state.cache.get_mut(&key) {
                cached.used = now;
                let _ = reply.send(cached.value.clone());
                return Ok(receiver);
            }
            if state.waiter_count() >= WAITERS {
                state.blocked_waiters = true;
                return Err(IconError::Busy);
            }
            if let Some(job) = state.jobs.get_mut(&key) {
                if let Some(old) = job.replies.insert(request, reply) {
                    let _ = old.send(Err(IconError::Cancelled));
                }
                return Ok(receiver);
            }
            if state.queue.len() >= QUEUED_LOADS {
                state.blocked_queue = true;
                return Err(IconError::Busy);
            }
            state.jobs.insert(
                key.clone(),
                Job {
                    active: false,
                    path,
                    replies: HashMap::from([(request, reply)]),
                },
            );
            state.queue.push_back(key);
        }
        self.start_jobs(&ready);
        Ok(receiver)
    }

    fn cancel(&self, request: &Request) -> bool {
        let mut state = self.0.state.lock().unwrap();
        state.jobs.retain(|_, job| {
            if let Some(reply) = job.replies.remove(request) {
                let _ = reply.send(Err(IconError::Cancelled));
            }
            job.active || !job.replies.is_empty()
        });
        let State { jobs, queue, .. } = &mut *state;
        queue.retain(|key| jobs.contains_key(key));
        state.take_capacity_notification()
    }

    pub fn cancel_window(&self, window: &str, ready: impl Fn()) {
        let requests: Vec<_> = self
            .0
            .state
            .lock()
            .unwrap()
            .jobs
            .values()
            .flat_map(|job| job.replies.keys())
            .filter(|request| request.window == window)
            .cloned()
            .collect();
        let mut notify = false;
        for request in requests {
            notify |= self.cancel(&request);
        }
        if notify {
            ready();
        }
    }

    fn start_jobs(&self, ready: &Arc<Notify>) {
        let mut work = Vec::new();
        let notify = {
            let mut state = self.0.state.lock().unwrap();
            while state.active < ACTIVE_LOADS {
                let Some(key) = state.queue.pop_front() else {
                    break;
                };
                let Some(job) = state.jobs.get_mut(&key) else {
                    continue;
                };
                job.active = true;
                let path = job.path.clone();
                state.active += 1;
                work.push((key, path));
            }
            state.take_capacity_notification()
        };
        if notify {
            ready();
        }
        for (key, path) in work {
            let store = self.clone();
            let ready = ready.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (store.0.loader)(&path, key.pixels)
                }))
                .ok()
                .flatten()
                .map(Arc::new)
                .ok_or(IconError::Unavailable);
                store.complete(key, value, ready);
            });
        }
    }

    fn complete(&self, key: Key, value: Reply, ready: Arc<Notify>) {
        let replies = {
            let mut state = self.0.state.lock().unwrap();
            state.active -= 1;
            let replies = state
                .jobs
                .remove(&key)
                .map(|job| job.replies)
                .unwrap_or_default();
            let bytes = value.as_ref().map_or(0, |bytes| bytes.len());
            if !replies.is_empty()
                && state.catalog.contains_key(&key.source)
                && bytes <= self.0.budget
            {
                while state.bytes + bytes > self.0.budget || state.cache.len() >= CACHE_ENTRIES {
                    let Some(oldest) = state
                        .cache
                        .iter()
                        .min_by_key(|(_, entry)| entry.used)
                        .map(|(key, _)| key.clone())
                    else {
                        break;
                    };
                    state.bytes -= state.cache.remove(&oldest).unwrap().bytes();
                }
                state.clock += 1;
                let used = state.clock;
                state.bytes += bytes;
                state.cache.insert(
                    key,
                    Cached {
                        value: value.clone(),
                        used,
                    },
                );
            }
            replies
        };
        for (_, reply) in replies {
            let _ = reply.send(value.clone());
        }
        self.start_jobs(&ready);
    }
}

#[tauri::command]
pub async fn app_icon(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    key: String,
    pixels: u16,
    request: String,
) -> Result<String, IconError> {
    if !window.is_visible().unwrap_or(false) {
        return Err(IconError::Cancelled);
    }
    let store = &app.state::<super::LauncherState>().icons;
    store
        .image(
            key,
            pixels,
            Request {
                window: window.label().into(),
                id: request,
            },
            Arc::new({
                let app = app.clone();
                move || {
                    let _ = app.emit("app-icons-ready", ());
                }
            }),
        )
        .await
}

#[tauri::command]
pub fn cancel_app_icon(app: tauri::AppHandle, window: tauri::WebviewWindow, request: String) {
    app.state::<super::LauncherState>().icons.cancel_request(
        &Request {
            window: window.label().into(),
            id: request,
        },
        || {
            let _ = app.emit("app-icons-ready", ());
        },
    );
}

#[cfg(test)]
#[path = "icon_tests.rs"]
mod tests;
