#![allow(dead_code)]

use std::path::Path;

pub fn single_line(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn desktop_exec_arg(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' | '\\' | '$' | '`' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn desktop_entry(name: &str, exe: &Path, folder: &str, icon: &str) -> String {
    let name = single_line(name);
    let exec = format!(
        "{} --launch {}",
        desktop_exec_arg(&exe.to_string_lossy()),
        desktop_exec_arg(folder),
    );

    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={name}\n\
         Comment=Launch {name} with OneClient\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Game;\n\
         StartupNotify=true\n"
    )
}

pub fn url_handler_entry(exe: &Path, scheme: &str, icon: &str) -> String {
    let exec = desktop_exec_arg(&exe.to_string_lossy());

    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=OneClient\n\
         Exec={exec} %u\n\
         Icon={icon}\n\
         Terminal=false\n\
         NoDisplay=true\n\
         MimeType=x-scheme-handler/{scheme};\n"
    )
}

pub fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

pub fn shell_script(exe: &Path, folder: &str) -> String {
    format!(
        "#!/bin/sh\nexec {} --launch {}\n",
        sh_quote(&exe.to_string_lossy()),
        sh_quote(folder),
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn bundle_slug(folder: &str) -> String {
    let slug: String = folder
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "cluster".to_string()
    } else {
        trimmed
    }
}

pub fn info_plist(name: &str, executable: &str, folder: &str, icon: Option<&str>) -> String {
    let display = xml_escape(&single_line(name));
    let icon_entry = icon.map_or_else(String::new, |icon| {
        format!("\t<key>CFBundleIconFile</key>\n\t<string>{}</string>\n", xml_escape(icon))
    });

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>CFBundleName</key>\n\t<string>{display}</string>\n\
         \t<key>CFBundleDisplayName</key>\n\t<string>{display}</string>\n\
         \t<key>CFBundleIdentifier</key>\n\t<string>org.polyfrost.OneClient.shortcut.{slug}</string>\n\
         \t<key>CFBundleExecutable</key>\n\t<string>{executable}</string>\n\
         {icon_entry}\
         \t<key>CFBundlePackageType</key>\n\t<string>APPL</string>\n\
         \t<key>CFBundleInfoDictionaryVersion</key>\n\t<string>6.0</string>\n\
         \t<key>CFBundleShortVersionString</key>\n\t<string>1.0</string>\n\
         \t<key>LSUIElement</key>\n\t<true/>\n\
         </dict>\n\
         </plist>\n",
        executable = xml_escape(executable),
        slug = bundle_slug(folder),
    )
}

pub fn url_shortcut(url: &str, icon: &Path) -> String {
    format!(
        "[InternetShortcut]\r\nURL={url}\r\nIconFile={}\r\nIconIndex=0\r\n",
        icon.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_newline_in_a_cluster_name_cannot_forge_a_second_key() {
        let entry = desktop_entry(
            "Pack\nExec=/bin/sh",
            &PathBuf::from("/usr/bin/oneclient_app"),
            "pack",
            "oneclient_app",
        );
        assert!(!entry.contains("Exec=/bin/sh"));
        assert!(entry.contains("Name=Pack Exec=/bin/sh"));
    }

    #[test]
    fn a_space_in_the_install_path_stays_one_argument() {
        let entry = desktop_entry(
            "Pack",
            &PathBuf::from("/opt/One Client/oneclient_app"),
            "my pack",
            "oneclient_app",
        );
        assert!(entry.contains(r#"Exec="/opt/One Client/oneclient_app" --launch "my pack""#));
    }

    #[test]
    fn the_url_handler_quotes_an_install_path_with_a_space() {
        let entry = url_handler_entry(
            &PathBuf::from("/opt/One Client/oneclient_app"),
            "oneclient",
            "oneclient_app",
        );
        assert!(entry.contains(r#"Exec="/opt/One Client/oneclient_app" %u"#));
        assert!(entry.contains("MimeType=x-scheme-handler/oneclient;"));
    }

    #[test]
    fn the_url_handler_keeps_the_field_code_outside_the_quoted_path() {
        let entry = url_handler_entry(
            &PathBuf::from("/usr/bin/oneclient_app"),
            "oneclient",
            "oneclient_app",
        );
        assert!(entry.contains(r#"Exec="/usr/bin/oneclient_app" %u"#));
        assert!(!entry.contains("%%u"));
    }

    #[test]
    fn percent_is_doubled_for_the_desktop_spec() {
        assert_eq!(desktop_exec_arg("100%"), r#""100%%""#);
    }

    #[test]
    fn desktop_reserved_characters_are_escaped() {
        assert_eq!(desktop_exec_arg("a$b`c\"d"), r#""a\$b\`c\"d""#);
    }

    #[test]
    fn the_mac_wrapper_survives_a_quote_in_the_path() {
        let script = shell_script(&PathBuf::from("/Users/o'brien/OneClient"), "pack");
        assert!(script.contains(r"'/Users/o'\''brien/OneClient'"));
        assert!(script.starts_with("#!/bin/sh\n"));
    }

    #[test]
    fn the_plist_escapes_a_name_that_looks_like_markup() {
        let plist = info_plist("A & <B>", "launch", "pack", Some("icon"));
        assert!(plist.contains("<string>A &amp; &lt;B&gt;</string>"));
        assert!(!plist.contains("<B>"));
    }

    #[test]
    fn the_plist_omits_the_icon_key_when_there_is_no_icon() {
        let plist = info_plist("Pack", "launch", "pack", None);
        assert!(!plist.contains("CFBundleIconFile"));
    }

    #[test]
    fn a_bundle_id_keeps_only_what_it_is_allowed() {
        assert_eq!(bundle_slug("My Pack (1.8.9)"), "My-Pack--1-8-9");
        assert_eq!(bundle_slug("---"), "cluster");
    }

    #[test]
    fn the_url_shortcut_is_a_single_ini_section() {
        let file = url_shortcut(
            "oneclient://launch/My%20Pack",
            &PathBuf::from(r"C:\Program Files\OneClient\oneclient_app.exe"),
        );

        assert!(file.starts_with("[InternetShortcut]\r\n"));
        assert!(file.contains("\r\nURL=oneclient://launch/My%20Pack\r\n"));
        assert!(file.contains(r"IconFile=C:\Program Files\OneClient\oneclient_app.exe"));
        assert!(file.ends_with("IconIndex=0\r\n"));
    }
}
