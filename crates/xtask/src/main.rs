use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() -> anyhow::Result<()> {
    let cmd = env::args().nth(1).unwrap_or_else(|| "help".into());
    match cmd.as_str() {
        "stage" => stage(),
        _ => {
            println!("xtask stage");
            Ok(())
        }
    }
}

fn stage() -> anyhow::Result<()> {
    let plugins = [
        "nyanyabot-plugin-screenshot",
        "nyanyabot-plugin-blobserver",
        "nyanyabot-plugin-amiabot-bilibili",
        "nyanyabot-plugin-amiabot-pixiv",
        "nyanyabot-plugin-amiabot-gallery",
        "nyanyabot-plugin-amiabot-onebot-websocket-client",
        "nyanyabot-plugin-amiabot-query",
        "nyanyabot-plugin-amiabot-zeabur-status",
        "nyanyabot-plugin-amiabot-pjsk-account",
        "nyanyabot-plugin-amiabot-pjsk-bind",
        "nyanyabot-plugin-amiabot-pjsk-card",
        "nyanyabot-plugin-amiabot-pjsk-event",
        "nyanyabot-plugin-amiabot-pjsk-song",
        "nyanyabot-plugin-amiabot-pjsk-profile",
        "nyanyabot-plugin-amiabot-pjsk-b30",
    ];
    let mut args = vec!["build", "--release"];
    for p in plugins {
        args.push("-p");
        args.push(p);
    }
    let st = Command::new("cargo").args(&args).status()?;
    if !st.success() {
        anyhow::bail!("build failed");
    }
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let workspace = manifest_dir.parent().unwrap().parent().unwrap();
    let target = workspace.join("target/release");
    let out = workspace.join("plugins");
    fs::create_dir_all(&out)?;
    for bin in plugins {
        let mut src = target.join(bin);
        let mut name = bin.to_string();
        if cfg!(windows) {
            src.set_extension("exe");
            name.push_str(".exe");
        }
        let dst = out.join(&name);
        fs::copy(&src, &dst)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;
        }
        println!("copied {}", dst.display());
    }
    Ok(())
}
