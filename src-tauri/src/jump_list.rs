// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows jump list tasks: New download, Pause all and Resume all.
use windows::{
    core::{Interface, HSTRING},
    Win32::{
        Storage::EnhancedStorage::PKEY_Title,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, StructuredStorage::PROPVARIANT,
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{
            Common::{IObjectArray, IObjectCollection},
            DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW,
            PropertiesSystem::IPropertyStore,
            ShellLink,
        },
    },
};

use crate::i18n::{text, Lang, Message};

/// The tasks in the order they appear in the jump list: the argument each one passes, and the
/// title shown in the app language.
fn tasks(lang: Lang) -> [(&'static str, &'static str); 3] {
    [
        ("--new", text(lang, Message::NewDownload)),
        ("--pause-all", text(lang, Message::PauseAll)),
        ("--resume-all", text(lang, Message::ResumeAll)),
    ]
}

/// Writes the jump list on a background thread. Failures are logged and ignored.
pub fn spawn(lang: Lang) {
    std::thread::spawn(move || {
        if let Err(err) = build(lang) {
            tracing::warn!("jump list was not updated: {err}");
        }
    });
}

fn build(lang: Lang) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let exe_text = exe.to_string_lossy().into_owned();
    // SAFETY: COM is initialized on this thread for the call and released after it.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = write_list(lang, &exe_text).map_err(|err| err.to_string());
    if initialized {
        // SAFETY: balances the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    result
}

fn write_list(lang: Lang, exe: &str) -> windows::core::Result<()> {
    // SAFETY: all COM objects are created and used on this thread only, and each call gets
    // valid arguments.
    unsafe {
        let list: ICustomDestinationList =
            CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        let mut max_slots = 0u32;
        let _removed: IObjectArray = list.BeginList(&mut max_slots)?;
        let collection: IObjectCollection =
            CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        for (argument, title) in tasks(lang) {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&HSTRING::from(exe))?;
            link.SetArguments(&HSTRING::from(argument))?;
            link.SetIconLocation(&HSTRING::from(exe), 0)?;
            let store: IPropertyStore = link.cast()?;
            let value = PROPVARIANT::from(title);
            store.SetValue(&PKEY_Title, &value)?;
            store.Commit()?;
            collection.AddObject(&link)?;
        }
        list.AddUserTasks(&collection)?;
        list.CommitList()?;
    }
    Ok(())
}
