use std::{
    fs,
    io::ErrorKind,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use notify::{RecursiveMode, Watcher};

use crate::{
    actions::Runtime,
    automation::WorkflowRegistry,
    config::Config,
    daemon::ipc::{
        DaemonRequest, DaemonResponse, DaemonStatus, PROTOCOL_VERSION, ResponseEnvelope,
        read_request, runtime_root, write_response,
    },
};

enum Job {
    Request {
        request: DaemonRequest,
        response: Sender<std::result::Result<DaemonResponse, String>>,
    },
    Reload,
}

struct SocketGuard(PathBuf);

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn serve(config: Config) -> Result<()> {
    let runtime_directory = runtime_root().join("xretype");
    ensure_private_directory(&runtime_directory)?;
    let socket = runtime_directory.join("xretyped.sock");
    remove_stale_socket(&socket)?;
    let listener = UnixListener::bind(&socket)
        .with_context(|| format!("could not bind daemon socket {}", socket.display()))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
        .context("could not protect daemon socket")?;
    let _socket_guard = SocketGuard(socket);

    let automation_path = config.automation_file();
    let registry = WorkflowRegistry::load(&automation_path)?;
    let status = Arc::new(Mutex::new(DaemonStatus {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        pid: std::process::id(),
        config_generation: 1,
        workflow_count: registry.workflow_count(),
        ..DaemonStatus::default()
    }));
    let (job_tx, job_rx) = mpsc::channel();
    spawn_worker(
        config,
        automation_path.clone(),
        registry,
        job_rx,
        Arc::clone(&status),
    );
    spawn_watcher(automation_path, job_tx.clone())?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx = job_tx.clone();
                let status = Arc::clone(&status);
                thread::spawn(move || handle_connection(stream, tx, status));
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("daemon socket accept failed"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: UnixStream, jobs: Sender<Job>, status: Arc<Mutex<DaemonStatus>>) {
    let response = match read_request(&mut stream) {
        Ok(envelope) if envelope.version == PROTOCOL_VERSION => {
            let (response_tx, response_rx) = mpsc::channel();
            status.lock().expect("status mutex poisoned").queue_depth += 1;
            if jobs
                .send(Job::Request {
                    request: envelope.request,
                    response: response_tx,
                })
                .is_err()
            {
                Err("xretyped execution worker stopped".to_owned())
            } else {
                response_rx
                    .recv()
                    .unwrap_or_else(|_| Err("xretyped execution worker stopped".to_owned()))
            }
        }
        Ok(envelope) => Err(format!(
            "protocol version mismatch: client {}, server {}",
            envelope.version, PROTOCOL_VERSION
        )),
        Err(error) => Err(format!("invalid daemon request: {error:#}")),
    };
    let _ = write_response(
        &mut stream,
        &ResponseEnvelope {
            version: PROTOCOL_VERSION,
            result: response,
        },
    );
}

fn spawn_worker(
    config: Config,
    automation_path: PathBuf,
    initial_registry: WorkflowRegistry,
    jobs: Receiver<Job>,
    status: Arc<Mutex<DaemonStatus>>,
) {
    thread::spawn(move || {
        let mut runtime = Runtime::new(config);
        let mut registry = initial_registry;
        while let Ok(job) = jobs.recv() {
            match job {
                Job::Reload => reload_registry(&automation_path, &mut registry, &status),
                Job::Request { request, response } => {
                    {
                        let mut current = status.lock().expect("status mutex poisoned");
                        current.queue_depth = current.queue_depth.saturating_sub(1);
                        current.active_workflow = match &request {
                            DaemonRequest::Run { workflow, .. } => Some(workflow.clone()),
                            _ => None,
                        };
                    }
                    let result = match request {
                        DaemonRequest::Execute(action) => {
                            runtime.execute(action).map(|_| DaemonResponse::Done)
                        }
                        DaemonRequest::Run {
                            workflow,
                            parameters,
                        } => registry
                            .execute_strings(&workflow, &parameters, &mut runtime)
                            .map(|_| DaemonResponse::Done),
                        DaemonRequest::Reload => {
                            reload_registry(&automation_path, &mut registry, &status);
                            let current = status.lock().expect("status mutex poisoned");
                            match &current.last_reload_error {
                                Some(error) => Err(anyhow::anyhow!(error.clone())),
                                None => Ok(DaemonResponse::Done),
                            }
                        }
                        DaemonRequest::Status => {
                            let mut snapshot =
                                status.lock().expect("status mutex poisoned").clone();
                            snapshot.active_overlay = crate::visual::active_overlay_name();
                            Ok(DaemonResponse::Status(snapshot))
                        }
                    }
                    .map_err(|error| format!("{error:#}"));
                    status
                        .lock()
                        .expect("status mutex poisoned")
                        .active_workflow = None;
                    let _ = response.send(result);
                }
            }
        }
    });
}

fn reload_registry(
    path: &Path,
    registry: &mut WorkflowRegistry,
    status: &Arc<Mutex<DaemonStatus>>,
) {
    match WorkflowRegistry::load(path) {
        Ok(candidate) => {
            let count = candidate.workflow_count();
            *registry = candidate;
            let mut current = status.lock().expect("status mutex poisoned");
            current.config_generation += 1;
            current.workflow_count = count;
            current.last_reload_error = None;
        }
        Err(error) => {
            status
                .lock()
                .expect("status mutex poisoned")
                .last_reload_error = Some(format!("{error:#}"));
        }
    }
}

fn spawn_watcher(path: PathBuf, jobs: Sender<Job>) -> Result<()> {
    let watch_path = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&watch_path).with_context(|| {
        format!(
            "could not create automation directory {}",
            watch_path.display()
        )
    })?;
    let (events_tx, events_rx) = mpsc::channel();
    let target_path = path.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if event_changes_target(&event.kind, &event.paths, &target_path) {
            let _ = events_tx.send(());
        }
    })
    .context("could not create automation file watcher")?;
    watcher
        .watch(&watch_path, RecursiveMode::NonRecursive)
        .with_context(|| format!("could not watch {}", watch_path.display()))?;
    thread::spawn(move || {
        let _watcher = watcher;
        while events_rx.recv().is_ok() {
            thread::sleep(Duration::from_millis(150));
            while events_rx.try_recv().is_ok() {}
            if jobs.send(Job::Reload).is_err() {
                break;
            }
        }
    });
    Ok(())
}

fn event_changes_target(kind: &notify::EventKind, paths: &[PathBuf], target: &Path) -> bool {
    paths.iter().any(|path| path == target)
        && matches!(
            kind,
            notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_)
        )
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("could not create runtime directory {}", path.display()))?;
    let metadata = fs::metadata(path)?;
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid {
        bail!(
            "runtime directory {} is not owned by the current user",
            path.display()
        );
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn remove_stale_socket(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if UnixStream::connect(path).is_ok() {
        bail!("xretyped is already running at {}", path.display());
    }
    fs::remove_file(path)
        .with_context(|| format!("could not remove stale socket {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    use notify::{
        EventKind,
        event::{AccessKind, ModifyKind},
    };

    use super::{ensure_private_directory, event_changes_target};

    #[test]
    fn runtime_directory_is_user_only() {
        let path = std::env::temp_dir().join(format!("xretyped-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        ensure_private_directory(&path).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn watcher_ignores_reads_and_unrelated_files() {
        let target = std::path::Path::new("/config/automations.yml");
        assert!(!event_changes_target(
            &EventKind::Access(AccessKind::Any),
            &[target.to_path_buf()],
            target
        ));
        assert!(!event_changes_target(
            &EventKind::Modify(ModifyKind::Any),
            &[std::path::PathBuf::from("/config/other.yml")],
            target
        ));
        assert!(event_changes_target(
            &EventKind::Modify(ModifyKind::Any),
            &[target.to_path_buf()],
            target
        ));
    }
}
