use crate::launcher::transfer::{DragCompletion, DragOutcome};
use crate::launcher::transfer::{ShareCompletion, ShareItem};
use std::path::PathBuf;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use windows::{
    ApplicationModel::DataTransfer::{
        DataRequestedEventArgs, DataTransferManager, TargetApplicationChosenEventArgs,
    },
    Foundation::{TypedEventHandler, Uri},
    Storage::{IStorageItem, StorageFile, StorageFolder},
    Win32::{
        Foundation::E_POINTER,
        System::WinRT::{
            RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
        },
        UI::Shell::IDataTransferManagerInterop,
    },
    core::Interface,
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

struct ShareSession {
    id: u64,
    manager: DataTransferManager,
    requested: i64,
    chosen: i64,
    data_ready: bool,
    target_chosen: bool,
    lost_focus: bool,
    complete: ShareCompletion,
    _apartment: Apartment,
}
thread_local! { static SHARE: RefCell<Option<ShareSession>> = const { RefCell::new(None) }; }
static NEXT_SHARE: AtomicU64 = AtomicU64::new(1);

fn finish_share(id: u64, result: Result<(), String>) {
    let session = SHARE.with(|current| {
        let mut current = current.borrow_mut();
        if current.as_ref().is_some_and(|session| session.id == id) {
            current.take()
        } else {
            None
        }
    });
    if let Some(session) = session {
        let _ = session.manager.RemoveDataRequested(session.requested);
        let _ = session
            .manager
            .RemoveTargetApplicationChosen(session.chosen);
        (session.complete)(result);
    }
}

fn share_progress(id: u64, data_ready: bool) {
    let finished = SHARE.with(|current| {
        let mut current = current.borrow_mut();
        let Some(session) = current.as_mut().filter(|session| session.id == id) else {
            return false;
        };
        if data_ready {
            session.data_ready = true;
        } else {
            session.target_chosen = true;
        }
        session.data_ready && session.target_chosen
    });
    if finished {
        finish_share(id, Ok(()));
    }
}

pub fn share_focus_changed(focused: bool) {
    let closed = SHARE.with(|current| {
        let mut current = current.borrow_mut();
        let session = current.as_mut()?;
        if !focused {
            session.lost_focus = true;
            return None;
        }
        session.lost_focus.then_some(session.id)
    });
    if let Some(id) = closed {
        finish_share(id, Ok(()));
    }
}

pub fn share(
    window: &tauri::WebviewWindow,
    item: ShareItem,
    complete: ShareCompletion,
) -> Result<(), String> {
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.map_err(|e| e.to_string())?;
    let apartment = Apartment;
    let hwnd = HWND(window.hwnd().map_err(|e| e.to_string())?.0);
    let interop: IDataTransferManagerInterop =
        windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()
            .map_err(|e| e.to_string())?;
    let manager: DataTransferManager =
        unsafe { interop.GetForWindow(hwnd) }.map_err(|e| e.to_string())?;
    let id = NEXT_SHARE.fetch_add(1, Ordering::Relaxed);
    let event_window = window.clone();
    let item = Arc::new(item);
    let preparing = Arc::new(AtomicBool::new(false));
    let on_data =
        TypedEventHandler::<DataTransferManager, DataRequestedEventArgs>::new(move |_, args| {
            let request = args
                .as_ref()
                .ok_or_else(|| windows::core::Error::from_hresult(E_POINTER))?
                .Request()?;
            if preparing.swap(true, Ordering::AcqRel) {
                request.FailWithDisplayText(&HSTRING::from(
                    "This share request is already being prepared.",
                ))?;
                return Ok(());
            }
            let deferral = request.GetDeferral()?;
            let item = item.clone();
            let event_window = event_window.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let result = (|| -> windows::core::Result<()> {
                    unsafe {
                        RoInitialize(RO_INIT_MULTITHREADED)?;
                    }
                    let _apartment = Apartment;
                    let data = request.Data()?;
                    data.Properties()?
                        .SetTitle(&HSTRING::from("Share from TinyDash"))?;
                    match &*item {
                        ShareItem::Text(text) => data.SetText(&HSTRING::from(text))?,
                        ShareItem::Url(url) => {
                            data.SetWebLink(&Uri::CreateUri(&HSTRING::from(url))?)?
                        }
                        ShareItem::File(path) => {
                            let name = HSTRING::from(path.as_os_str());
                            let file: IStorageItem = if path.is_dir() {
                                StorageFolder::GetFolderFromPathAsync(&name)?
                                    .join()?
                                    .cast()?
                            } else {
                                StorageFile::GetFileFromPathAsync(&name)?.join()?.cast()?
                            };
                            let items: windows_collections::IIterable<IStorageItem> =
                                vec![Some(file)].into();
                            data.SetStorageItems(&items, true)?;
                        }
                    }
                    Ok(())
                })()
                .map_err(|error| error.to_string());
                if result.is_err() {
                    let _ = request.FailWithDisplayText(&HSTRING::from(
                        "TinyDash could not prepare this item. Refresh the result and try again.",
                    ));
                }
                let _ = deferral.Complete();
                let _ = event_window.run_on_main_thread(move || match result {
                    Ok(()) => share_progress(id, true),
                    Err(error) => finish_share(id, Err(error)),
                });
            });
            Ok(())
        });
    let requested = manager.DataRequested(&on_data).map_err(|e| e.to_string())?;
    let on_chosen = TypedEventHandler::<DataTransferManager, TargetApplicationChosenEventArgs>::new(
        move |_, _| {
            share_progress(id, false);
            Ok(())
        },
    );
    let chosen = match manager.TargetApplicationChosen(&on_chosen) {
        Ok(token) => token,
        Err(error) => {
            let _ = manager.RemoveDataRequested(requested);
            return Err(error.to_string());
        }
    };
    SHARE.with(|current| {
        *current.borrow_mut() = Some(ShareSession {
            id,
            manager,
            requested,
            chosen,
            data_ready: false,
            target_chosen: false,
            lost_focus: false,
            complete,
            _apartment: apartment,
        })
    });
    if let Err(error) = unsafe { interop.ShowShareUIForWindow(hwnd) } {
        finish_share(id, Err(error.to_string()));
    }
    Ok(())
}
use windows::{
    Win32::{
        Foundation::HWND,
        System::{
            Com::IDataObject,
            Ole::{DROPEFFECT_COPY, IDropSource},
        },
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
            Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop},
        },
    },
    core::HSTRING,
};

pub fn drag(
    window: &tauri::WebviewWindow,
    path: PathBuf,
    complete: DragCompletion,
) -> Result<(), String> {
    if unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } >= 0 {
        complete(Ok(DragOutcome::Cancelled));
        return Ok(());
    }
    let hwnd = HWND(window.hwnd().map_err(|e| e.to_string())?.0);
    let path = HSTRING::from(path.as_os_str());
    let effect = unsafe {
        // Shell-owned COM objects supply file formats and the default drag image.
        let item: IShellItem =
            SHCreateItemFromParsingName(&path, None).map_err(|e| e.to_string())?;
        let data: IDataObject = item
            .BindToHandler(None, &BHID_DataObject)
            .map_err(|e| e.to_string())?;
        SHDoDragDrop(Some(hwnd), &data, None::<&IDropSource>, DROPEFFECT_COPY)
    }
    .map_err(|e| e.to_string())?;
    complete(Ok(if effect.0 == 0 {
        DragOutcome::Cancelled
    } else {
        DragOutcome::Dropped
    }));
    Ok(())
}
