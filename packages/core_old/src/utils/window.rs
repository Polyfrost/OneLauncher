use tauri::WebviewWindow;

pub fn set_window_styling(
	win: &WebviewWindow,
	custom: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	win.set_decorations(!custom)?;

	#[cfg(target_os = "macos")]
	{
		if custom {
			win.set_title_bar_style(tauri::TitleBarStyle::Overlay)?;
		} else {
			win.set_title_bar_style(tauri::TitleBarStyle::Visible)?;
		}
	}

	Ok(())
}
