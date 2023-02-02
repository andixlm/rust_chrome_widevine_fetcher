use const_format::formatcp;
use std::io::Write;

const CHROME_WIDEVINE_PATH: &'static str = 
    "/Volumes/Google Chrome/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Libraries/WidevineCdm";

const CHROMIUM_LIBRARIES_PATH: &'static str =
    "/Applications/Chromium.app/Contents/Frameworks/Chromium Framework.framework/Libraries";

const CHROMIUM_WIDEVINE_PATH: &'static str = formatcp!("{}/WidevineCdm", CHROMIUM_LIBRARIES_PATH);

async fn print_progress(
    expected_size: usize,
    fetched_size: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    let mut fetched_so_far = fetched_size.load(std::sync::atomic::Ordering::SeqCst);

    while fetched_so_far < expected_size {
        println!(
            "Fetched {} bytes so far, {}%",
            fetched_so_far,
            (fetched_so_far as f32 / expected_size as f32) * 100f32
        );

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        fetched_so_far = fetched_size.load(std::sync::atomic::Ordering::SeqCst);
    }

    println!(
        "Fetched {} bytes, {}%",
        fetched_so_far,
        (fetched_so_far as f32 / expected_size as f32) * 100f32
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chromium_libraries = std::fs::metadata(CHROMIUM_LIBRARIES_PATH)
        .expect("Unable to find Chromium libraries, looks like Chromium is not installed");

    assert!(chromium_libraries.is_dir());

    let client = reqwest::Client::new();

    let mut image = client
        .get("https://dl.google.com/chrome/mac/universal/stable/GGRO/googlechrome.dmg")
        .send()
        .await?;

    assert_eq!(image.status(), reqwest::StatusCode::OK);

    let expected_size: usize = image
        .content_length()
        .expect("Unknown size of data to be fetched")
        .try_into()
        .unwrap();

    let has_file = if let Ok(metadata) = std::fs::metadata("/tmp/googlechrome.dmg") {
        metadata.len() as usize == expected_size
    } else {
        false
    };

    if has_file {
        println!("Same image found in /tmp");
    } else {
        println!("{} bytes are expected to be fetched", expected_size);

        let fetched_size = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mut content = bytes::BytesMut::with_capacity(expected_size);
        let progress = tokio::spawn(print_progress(expected_size, fetched_size.clone()));

        while let Some(chunk) = image.chunk().await? {
            fetched_size.fetch_add(chunk.len(), std::sync::atomic::Ordering::SeqCst);

            content.extend(chunk);
        }

        progress.await?;

        println!(
            "Fetched {} bytes in total",
            fetched_size.load(std::sync::atomic::Ordering::SeqCst)
        );

        assert_eq!(content.len(), expected_size);

        tokio::task::spawn_blocking(move || {
            let mut file = std::fs::File::create("/tmp/googlechrome.dmg")?;

            file.write(&content)
        })
        .await??;
    }

    let mut stdout = std::io::stdout().lock();

    print!("Mounting image /tmp/googlechrome.dmg... ");
    stdout.flush()?;

    tokio::task::spawn_blocking(|| {
        std::process::Command::new("hdiutil")
            .arg("attach")
            .arg("-quiet")
            .arg("-nobrowse")
            .arg("/tmp/googlechrome.dmg")
            .spawn()
            .expect("Failed to start mounting /tmp/googlechrome.dmg")
            .wait()
            .expect("Failed to mount /tmp/googlechrome.dmg")
    })
    .await?;

    println!("OK");

    let chrome_widevine_metadata =
        std::fs::metadata(CHROME_WIDEVINE_PATH).expect("Unable to find Google Chrome Widevine");

    print!("Checking... ");
    stdout.flush()?;

    assert!(chrome_widevine_metadata.is_dir());

    println!("OK");

    print!("Copying... ");
    stdout.flush()?;

    tokio::task::spawn_blocking(|| {
        std::process::Command::new("cp")
            .arg("-R")
            .arg(CHROME_WIDEVINE_PATH)
            .arg(CHROMIUM_WIDEVINE_PATH)
            .spawn()
            .expect("Failed to start copying WidevineCdm")
            .wait()
            .expect("Failed to copy WidevineCdm")
    })
    .await?;

    println!("OK");

    print!("Unmounting... ");
    stdout.flush()?;

    tokio::task::spawn_blocking(|| {
        std::process::Command::new("hdiutil")
            .arg("detach")
            .arg("-quiet")
            .arg("-force")
            .arg("/Volumes/Google Chrome")
            .spawn()
            .expect("Failed to start unmounting /Volumes/Google Chrome")
            .wait()
            .expect("Failed to unmount Google Chrome")
    })
    .await?;

    println!("OK");

    print!("Removing... ");
    stdout.flush()?;

    tokio::task::spawn_blocking(|| {
        std::fs::remove_file("/tmp/googlechrome.dmg")
            .expect("Failed to remove /tmp/googlechrome.dmg")
    })
    .await?;

    println!("OK");

    Ok(())
}
