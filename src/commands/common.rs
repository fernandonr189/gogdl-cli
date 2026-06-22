pub async fn find_executables(base_path: &str) -> Vec<String> {
    let mut executables = Vec::new();

    if let Ok(_entries) = tokio::fs::read_dir(base_path).await {
        let mut stack = vec![String::new()];

        while let Some(relative_dir) = stack.pop() {
            let current_path = if relative_dir.is_empty() {
                base_path.to_string()
            } else {
                format!("{}/{}", base_path, relative_dir)
            };

            if let Ok(mut dir_entries) = tokio::fs::read_dir(&current_path).await {
                while let Ok(Some(entry)) = dir_entries.next_entry().await {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let relative_path = if relative_dir.is_empty() {
                        file_name.clone()
                    } else {
                        format!("{}/{}", relative_dir, file_name)
                    };

                    if let Ok(file_type) = entry.file_type().await {
                        if file_type.is_dir() {
                            // Skip common non-game directories
                            let skip_dirs = [
                                "support",
                                "__support",
                                "directx",
                                "redist",
                                "vcredist",
                                "_commonredist",
                            ];
                            if !skip_dirs
                                .iter()
                                .any(|d| file_name.to_lowercase().contains(d))
                            {
                                stack.push(relative_path);
                            }
                        } else if file_type.is_file() {
                            let lower = file_name.to_lowercase();
                            if lower.ends_with(".exe") {
                                // Skip common non-game executables
                                let skip_exes = [
                                    "unins",
                                    "setup",
                                    "install",
                                    "crash",
                                    "report",
                                    "vc_redist",
                                    "dxsetup",
                                    "dotnet",
                                ];
                                if !skip_exes.iter().any(|s| lower.contains(s)) {
                                    executables.push(relative_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    executables.sort();
    executables.dedup();
    executables
}
