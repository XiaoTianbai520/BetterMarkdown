pub mod commands;
pub mod saf_android;
pub mod search;
pub mod storage_android;
pub mod watcher;
pub mod workspace_index;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_fs::init())
        .manage(watcher::WatcherState::new())
        .invoke_handler(tauri::generate_handler![
            // File I/O
            commands::read_file,
            commands::read_binary_file,
            commands::write_file,
            commands::write_binary_file,
            commands::copy_file,
            // Directory & filesystem
            commands::list_dir,
            commands::fs_create_file,
            commands::fs_create_dir,
            commands::fs_delete,
            commands::fs_dir_exists,
            commands::fs_rename,
            commands::fs_move,
            commands::fs_list_dirs,
            commands::fs_list_extensions,
            commands::fs_dirs_with_extensions,
            commands::webview_runtime_version,
            commands::update_frontmatter_property,
            commands::delete_frontmatter_property,
            search::search_in_dir,
            // Android SAF
            saf_android::saf_pick_folder,
            saf_android::saf_poll_picked,
            saf_android::saf_persisted_trees,
            saf_android::saf_tree_root,
            saf_android::saf_tree_name,
            saf_android::saf_list,
            saf_android::saf_read,
            saf_android::saf_read_base64,
            saf_android::saf_write,
            saf_android::saf_create,
            saf_android::saf_delete,
            // Android storage permissions
            storage_android::android_has_all_files_access,
            storage_android::android_request_all_files_access,
            storage_android::android_system_insets,
            storage_android::android_restart_app,
            // Watcher
            watcher::watch_file,
            watcher::unwatch_file,
            // Workspace index
            workspace_index::workspace_index_init,
            workspace_index::workspace_index_files,
            workspace_index::workspace_index_backlinks,
            workspace_index::workspace_index_tags,
            workspace_index::workspace_index_resolve,
            workspace_index::workspace_index_rescan,
            workspace_index::workspace_index_referenced_by,
        ]);

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_, _| {});
}
