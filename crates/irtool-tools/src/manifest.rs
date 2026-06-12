use std::collections::HashMap;
use std::path::Path;

/// Verification method for downloaded tools.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum VerifyMethod {
    /// No verification (for tools from unversioned URLs or dev use)
    #[default]
    None,
    /// Verify Authenticode signature + publisher via WinVerifyTrust
    Authenticode,
    /// Verify exact SHA256 hash (for version-pinned tools)
    Sha256,
}

/// Manifest for a single external tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ToolManifest {
    pub version: String,
    pub url: String,
    #[serde(default)]
    pub verify: VerifyMethod,
    #[serde(default)]
    pub expected_signer: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    pub files: Vec<String>,
    #[serde(default)]
    pub optional: bool,
}

/// All tool manifests, keyed by tool id.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolManifests {
    #[serde(flatten)]
    tools: HashMap<String, ToolManifest>,
}

/// Installed tool version entry for manifest.json in the tools directory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct InstalledToolEntry {
    version: String,
}

/// Installed tools manifest, written to <tools_dir>/manifest.json after download.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct InstalledManifest {
    #[serde(flatten)]
    tools: HashMap<String, InstalledToolEntry>,
}

/// Write/update the installed manifest.json in the tools directory.
/// Reads existing manifest to preserve other tools' versions, then updates the entry for `tool_id`.
pub fn write_installed_manifest(
    tools_dir: &Path,
    tool_id: &str,
    version: &str,
) -> Result<(), std::io::Error> {
    let manifest_path = tools_dir.join("manifest.json");

    let mut installed = if manifest_path.exists() {
        let content = std::fs::read_to_string(&manifest_path)?;
        serde_json::from_str::<InstalledManifest>(&content).unwrap_or_else(|_| InstalledManifest {
            tools: HashMap::new(),
        })
    } else {
        InstalledManifest {
            tools: HashMap::new(),
        }
    };

    installed.tools.insert(
        tool_id.to_string(),
        InstalledToolEntry {
            version: version.to_string(),
        },
    );

    let json = serde_json::to_string_pretty(&installed)?;
    std::fs::write(&manifest_path, json)?;
    Ok(())
}

impl ToolManifests {
    /// Load manifests from embedded default.
    pub fn load() -> Self {
        let default = include_str!("../manifest.json");
        serde_json::from_str(default)
            .expect("内置 manifest.json 格式错误，这是编译期 bug")
    }

    pub fn get(&self, id: &str) -> Option<&ToolManifest> {
        self.tools.get(id)
    }

    pub fn ids(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Check which tools are installed / missing.
    pub fn check(&self, tools_dir: &Path) -> Vec<super::ToolStatus> {
        self.tools
            .iter()
            .map(|(id, manifest)| {
                let tool_dir = tools_dir.join(id);
                let missing: Vec<String> = manifest
                    .files
                    .iter()
                    .filter(|f| !tool_dir.join(f).exists())
                    .cloned()
                    .collect();
                let installed = missing.is_empty();
                super::ToolStatus {
                    id: id.clone(),
                    installed,
                    version: if installed {
                        Some(manifest.version.clone())
                    } else {
                        None
                    },
                    files: manifest.files.clone(),
                    missing_files: missing,
                    optional: manifest.optional,
                }
            })
            .collect()
    }
}
