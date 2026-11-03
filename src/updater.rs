use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{env, fs, os::unix::fs::PermissionsExt, process::Command};

/// Repositório público de onde as atualizações são baixadas.
pub const REPO: &str = "renanrmsantos14/pulseforge";
/// Nome do binário anexado em cada release do GitHub.
pub const ASSET_NAME: &str = "pulseforge-linux-x86_64";

#[derive(Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

/// Extrai versão e URL do binário do JSON de `releases/latest` da API do GitHub.
pub fn parse_release(json: &str) -> Result<Release> {
    let value: Value = serde_json::from_str(json).context("resposta inválida do GitHub")?;
    let tag = value["tag_name"]
        .as_str()
        .context("release sem tag_name")?;
    let url = value["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|asset| asset["name"] == ASSET_NAME)
        .and_then(|asset| asset["browser_download_url"].as_str())
        .with_context(|| format!("release {tag} sem o arquivo {ASSET_NAME}"))?;
    Ok(Release {
        version: tag.trim_start_matches('v').to_owned(),
        url: url.to_owned(),
    })
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| {
        v.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(candidate) > parse(current)
}

fn curl(args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", "--max-time", "120"])
        .args(args)
        .output()
        .context("curl não encontrado; instale com: sudo apt install curl")?;
    if !output.status.success() {
        bail!(
            "falha ao acessar o GitHub: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn latest_release() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = curl(&[
        "--header",
        "Accept: application/vnd.github+json",
        "--header",
        "User-Agent: pulseforge-updater",
        &url,
    ])?;
    parse_release(&String::from_utf8_lossy(&body))
}

/// Consulta a última release no GitHub e, se for mais nova, substitui o binário em uso.
pub fn update() -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let release = latest_release()?;
    if !is_newer(&release.version, current_version) {
        bail!("já está na versão mais recente ({current_version})");
    }
    let current = env::current_exe()?;
    let tmp = current.with_extension("update");
    let download = curl(&["--header", "User-Agent: pulseforge-updater", &release.url])?;
    fs::write(&tmp, download).context("não foi possível salvar a atualização")?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    let check = Command::new(&tmp).arg("--version").output();
    let valid = check
        .as_ref()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(&release.version))
        .unwrap_or(false);
    if !valid {
        let _ = fs::remove_file(&tmp);
        bail!("o arquivo baixado não é o PulseForge {}", release.version);
    }
    fs::rename(&tmp, &current).context("não foi possível instalar a atualização")?;
    println!("PulseForge atualizado de {current_version} para {}", release.version);
    Ok(())
}
