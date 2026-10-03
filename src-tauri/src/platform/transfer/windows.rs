use crate::launcher::transfer::{DragCompletion, DragOutcome};
use std::path::PathBuf;
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
