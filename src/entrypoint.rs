use {
    crate::geyser_plugin_hook::GeyserPluginHook,
    solana_geyser_plugin_interface::geyser_plugin_interface::GeyserPlugin,
};

#[no_mangle]
#[allow(improper_ctypes_definitions)]
/// # Safety
///
/// This function simply allocates a GeyserPluginHook,
/// and returns a pointer to it as trait GeyserPlugin.
pub unsafe extern "C" fn _create_plugin() -> *mut dyn GeyserPlugin {
    let plugin = GeyserPluginHook::default();
    let plugin: Box<dyn GeyserPlugin> = Box::new(plugin);
    Box::into_raw(plugin)
}
