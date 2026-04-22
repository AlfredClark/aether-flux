pub mod app_shell;
pub mod backend_i18n;

#[macro_export]
macro_rules! app_shell_commands {
    ($callback:ident [$($acc:path,)*] $($rest:ident)*) => {
        $callback!(
            [
                $($acc,)*
                $crate::utils::app_shell::configure_asr_hotkey,
                $crate::utils::app_shell::set_tray_mode_enabled,
                $crate::utils::app_shell::get_tray_mode_enabled,
            ]
            $($rest)*
        )
    };
}
