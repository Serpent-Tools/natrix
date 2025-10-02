use natrix::prelude::*;

mod reload_tests;

const HELLO_TEXT: &str = "HELLO WORLD, TEST TEST!";
const HELLO_ID: Id = natrix::id!();
const PANIC_ID: Id = natrix::id!();
const BUTTON_ID: Id = natrix::id!();
const RELOAD_ID: Id = natrix::id!();
const IMG_ID: Id = natrix::id!();

#[derive(State)]
struct NotUsed;

#[expect(dead_code, reason = "literally here to test DCE")]
fn render_not_used() -> impl Element<NotUsed> {
    e::img()
}

#[derive(State)]
struct HelloWorld {
    counter: Signal<usize>,
}

fn render_hello_world() -> impl Element<HelloWorld> {
    e::div()
        .child(e::h1().text(HELLO_TEXT).id(HELLO_ID))
        .child(integration_tests_dependency::dep_component())
        .child(e::button().id(PANIC_ID).text("PANIC").on::<events::Click>(
            |mut _ctx: EventCtx<HelloWorld>, _| {
                panic!("Panic button clicked!");
            },
        ))
        .child(
            e::button()
                .id(BUTTON_ID)
                .on::<events::Click>(|mut ctx: EventCtx<HelloWorld>, _| {
                    *ctx.counter += 1;
                })
                .text(|ctx: RenderCtx<HelloWorld>| *ctx.counter), // .class(HELLO),
        )
        .child(e::div().id(RELOAD_ID).text(reload_tests::VALUE))
        .child(
            e::img()
                .src(natrix::asset!("../assets/logo.png"))
                .id(IMG_ID),
        )
}

fn main() {
    natrix::mount(
        HelloWorld {
            counter: Signal::new(0),
        },
        render_hello_world,
    );
}

#[cfg(test)]
mod driver_tests {
    use std::time::{Duration, Instant};

    use thirtyfour::common::config::WebDriverConfig;
    use thirtyfour::{By, ChromiumLikeCapabilities, DesiredCapabilities, WebDriver};
    use tokio::time::sleep;

    use crate::{BUTTON_ID, HELLO_ID, HELLO_TEXT, IMG_ID, PANIC_ID};

    async fn create_client() -> WebDriver {
        let url = if cfg!(feature = "build_test") {
            "http://page.local/dist/"
        } else {
            "http://page.local:3000"
        };

        let mut caps = DesiredCapabilities::chrome();
        caps.set_headless().unwrap();
        caps.set_no_sandbox().unwrap();
        caps.set_disable_gpu().unwrap();
        caps.set_disable_dev_shm_usage().unwrap();
        caps.set_disable_web_security().unwrap();

        caps.add_arg("--allow-insecure-localhost").unwrap();
        caps.add_arg("--ignore-certificate-errors").unwrap();
        caps.add_arg("--disable-features=AutoupgradeMixedContent")
            .unwrap();
        caps.add_arg("--unsafely-treat-insecure-origin-as-secure=http://page.local:3000")
            .unwrap();
        caps.add_arg("--disable-features=UseDnsHttpsSvcb").unwrap();
        caps.add_arg("--disable-async-dns").unwrap();
        caps.add_arg("--disable-features=DnsOverHttps").unwrap();

        let config = WebDriverConfig::builder()
            // .reqwest_timeout(Duration::from_secs(5))
            .build()
            .expect("Config invalid");

        let driver = WebDriver::new_with_config("http://chrome.local:8000", caps, config)
            .await
            .expect("Failed to connect to chrome driver");

        let start = Instant::now();
        loop {
            let res = driver.get(url).await;
            sleep(Duration::from_millis(100)).await;
            if res.is_ok() {
                break;
            } else if let Err(err) = res {
                println!("{err:?}");
            }
            if start.elapsed().as_secs() > 20 {
                panic!("Loading URL took too long");
            }
        }

        let start = Instant::now();
        let mut last_refresh = Instant::now();
        loop {
            let element = driver.find(By::Id(HELLO_ID.0)).await;
            sleep(Duration::from_millis(100)).await;
            if element.is_ok() {
                break;
            }
            if start.elapsed().as_secs() > 60 {
                panic!("Loading WASM took too long");
            } else if last_refresh.elapsed().as_secs() > 10 {
                println!("Loading WASM taking too long - trying a refresh");
                driver.get(url).await.unwrap();
                sleep(Duration::from_millis(1000)).await;
                last_refresh = Instant::now();
            }
        }

        driver
    }

    #[tokio::test]
    async fn loading_framework_works() {
        let client = create_client().await;
        let element = client.find(By::Id(HELLO_ID.0)).await.unwrap();
        let text = element.text().await.unwrap();
        assert_eq!(text, HELLO_TEXT);
    }

    #[tokio::test]
    #[ignore = "css in the middle of refactoring"]
    async fn primary_global_css() {
        let client = create_client().await;
        let element = client.find(By::Id(HELLO_ID.0)).await.unwrap();
        let text = element.css_value("background-color").await.unwrap();
        assert_eq!(text, "rgba(1, 2, 3, 1)");
    }

    #[tokio::test]
    async fn simple_dep() {
        let client = create_client().await;
        let element = client
            .find(By::Id(integration_tests_dependency::DEP_ID.0))
            .await
            .unwrap();
        let text = element.text().await.unwrap();
        assert_eq!(text, integration_tests_dependency::DEP_TEXT);
    }

    #[tokio::test]
    async fn panic_button() {
        let client = create_client().await;

        let panic_button = client.find(By::Id(PANIC_ID.0)).await.unwrap();
        let button = client.find(By::Id(BUTTON_ID.0)).await.unwrap();

        button.click().await.unwrap();
        let text = button.text().await.unwrap();
        assert_eq!(text, "1");

        panic_button.click().await.unwrap();
        client.accept_alert().await.unwrap();

        button.click().await.unwrap();
        let text = button.text().await.unwrap();
        assert_eq!(
            text, "1",
            "Panic should have prevented further rust execution"
        );
    }

    #[tokio::test]
    async fn assets() {
        let client = create_client().await;
        let element = client.find(By::Id(IMG_ID.0)).await.unwrap();
        let rect = element.rect().await.unwrap();
        let width = rect.width;

        assert!(width >= 100.0, "Img width too small {width}");
    }

    #[tokio::test]
    #[ignore = "Doesnt work in current CI environment"]
    #[cfg(not(feature = "build_test"))]
    async fn reload() {
        use crate::{RELOAD_ID, reload_tests};

        if option_env!("TEST_KIND_BUILD").is_some() {
            return;
        }

        let client = create_client().await;
        let element = client.find(By::Id(RELOAD_ID.0)).await.unwrap();

        let text = element.text().await.unwrap();
        assert_eq!(text, reload_tests::VALUE);

        let new_text = format!("{}E", reload_tests::VALUE);
        std::fs::write(
            "src/reload_tests.rs",
            format!("pub const VALUE: &str = \"{new_text}\";\n"),
        )
        .unwrap();

        // Wait for the file to be reloaded

        let start = Instant::now();
        loop {
            sleep(Duration::from_millis(100)).await;
            if let Ok(element) = client.find(By::Id(RELOAD_ID.0)).await
                && let Ok(text) = element.text().await
                && text == new_text
            {
                break;
            }

            if start.elapsed().as_secs() > 20 {
                panic!("Reloading took too long");
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "build_test")]
mod dist_tests {
    #[test]
    #[ignore = "Does not have access to `dist` in current CI"]
    fn duplicate_assets_calls() {
        let mut amount_logo = 0;
        for file in std::fs::read_dir("./dist").unwrap() {
            let file = file.unwrap();
            if file.file_name().to_str().unwrap().contains("logo.png") {
                amount_logo += 1;
            }
        }

        assert_eq!(
            amount_logo, 1,
            "There should be only one logo.png in the dist folder, even if included multiple times"
        );
    }
}
