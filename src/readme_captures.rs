//! Reproducible, offline README captures. All paths, files and transfers are synthetic.
use crate::*;

fn demo() -> App {
    let mut app = App::new(
        r"C:\Demo\Projects".into(),
        r"D:\Archive\Projects".into(),
        vec![],
    );
    app.locations = locations::Locations::screenshot_demo();
    app.motion = motion::Policy::DISABLED;
    app.elevated = false;
    app.sidebar_width = 224.;
    app.window_width = 1920.;
    let folders = [
        r"C:\Demo\Projects",
        r"D:\Archive\Projects",
        r"C:\Demo\Design assets",
        r"E:\Media\Exports",
    ];
    let names = [
        ("Design assets", true, 840_000_000),
        ("Documents", true, 320_000_000),
        ("Media", true, 1_280_000_000),
        ("Source", true, 192_000_000),
        ("Release build.zip", false, 460_000_000),
        ("Brand guidelines.pdf", false, 24_000_000),
        ("Project overview.pdf", false, 12_400_000),
        ("Roadmap.xlsx", false, 3_200_000),
        ("Presentation.pptx", false, 38_000_000),
        ("README.md", false, 16_384),
        ("Meeting notes.txt", false, 8_192),
        ("Preview.png", false, 6_800_000),
        ("manifest.json", false, 4_096),
        ("Archive index.csv", false, 2_600_000),
    ];
    for (id, pane) in app.panes.iter_mut().enumerate() {
        pane.tabs = vec![
            Tab::new(folders[id].into()),
            Tab::new(r"C:\Demo\Documents".into()),
        ];
        pane.selected_tab = 0;
        pane.entries = names
            .iter()
            .enumerate()
            .map(|(i, &(name, directory, bytes))| Entry {
                path: pane.path.join(name),
                name: name.into(),
                directory,
                hidden: false,
                bytes: Some(bytes + id as u64 * 1024),
                partial: false,
                modified: None,
                modified_label: format!("2026-10-{:02} {:02}:30", 28 - i % 9, 9 + i % 6),
                icon: None,
            })
            .collect();
        pane.total_bytes = pane.entries.iter().filter_map(|e| e.bytes).sum();
        pane.reorder();
    }
    app.panes[0].selected = Some(0);
    app.panes[0].selections.insert(0);
    let mut job = transfers::Job::new(
        1,
        transfers::Request {
            kind: transfers::Kind::Copy,
            sources: vec![
                r"C:\Demo\Projects\Design assets".into(),
                r"C:\Demo\Projects\Media".into(),
            ],
            destination: Some(r"D:\Archive\Projects".into()),
        },
    );
    job.state = "Transferring";
    job.progress = transfers::Progress {
        total_bytes: 2_120_000_000,
        done_bytes: 1_356_800_000,
        total_items: 248,
        done_items: 159,
        bytes_per_second: 86_000_000.,
        eta: Some(9),
        status: r"Copying C:\Demo\Projects\Media\Product demo.mp4 → D:\Archive\Projects\Media"
            .into(),
        ..Default::default()
    };
    app.jobs.push_back(job);
    app.running_job = Some(1);
    app.queue_expanded = true;
    app.queue_height = 118.;
    app
}

fn save(app: &App, name: &str) {
    let pixels = final_ui_checks::render(app, iced::Size::new(1920, 1120), None);
    tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(1920, 1120).unwrap())
        .unwrap()
        .save_png(format!("target/readme-captures/{name}.png"))
        .unwrap();
}

#[test]
#[ignore = "explicit README screenshot generation with synthetic fixtures"]
fn generate_readme_gallery() {
    std::fs::create_dir_all("target/readme-captures").unwrap();
    let mut app = demo();
    for look in preferences::Look::ALL {
        app.preferences.look = look;
        for (mode, label) in [
            (preferences::ThemeMode::Dark, "dark"),
            (preferences::ThemeMode::Light, "light"),
        ] {
            app.preferences.theme = mode;
            let _ = app.apply_appearance();
            app.motion = motion::Policy::DISABLED;
            let name = if look == preferences::Look::Default {
                format!("file-beholder-{label}")
            } else {
                format!("file-beholder-{}-{label}", look.key())
            };
            save(&app, &name);
        }
    }
    app.preferences.look = preferences::Look::Default;
    app.preferences.theme = preferences::ThemeMode::Dark;
    let _ = app.apply_appearance();
    app.motion = motion::Policy::DISABLED;
    for (count, label) in [(3, "three-panels"), (4, "four-panels")] {
        app.pane_count = count;
        app.stacked = false;
        app.grid = true;
        app.rebuild_file_layout();
        save(&app, &format!("file-beholder-{label}"));
    }
}

#[test]
fn neumorphic_switch_tracks_are_pills_in_every_state() {
    for dark in [true, false] {
        let t = palette::styled(
            dark,
            Default::default(),
            preferences::TintMode::DarkOnly,
            preferences::Look::Neumorphic,
        );
        for states in [t.visual.switch_off.unwrap(), t.visual.switch_on.unwrap()] {
            assert_eq!(states.normal.shape, Some(sk_iced::visual::Shape::Pill));
            for surface in [
                states.hovered,
                states.pressed,
                states.disabled,
                states.focused,
            ]
            .into_iter()
            .flatten()
            {
                assert_eq!(surface.shape, Some(sk_iced::visual::Shape::Pill));
            }
        }
    }
}

#[test]
#[ignore = "visual preview of Neumorphic on/off switches"]
fn capture_neumorphic_switches() {
    for (mode, label) in [
        (preferences::ThemeMode::Dark, "dark"),
        (preferences::ThemeMode::Light, "light"),
    ] {
        let mut app = demo();
        app.preferences.look = preferences::Look::Neumorphic;
        app.preferences.theme = mode;
        let _ = app.apply_appearance();
        app.motion = motion::Policy::DISABLED;
        let t = palette::styled(
            app.dark,
            app.preferences.accent,
            app.preferences.tint,
            app.look(),
        );
        let appearance = sk_iced::components::Appearance::fixed(crate::typography::font(), t);
        let controls = iced::widget::row![
            text("Off"),
            sk_iced::components::ToggleSwitch::new(appearance, app.motion, false)
                .on_toggle(Message::SearchSubfolders),
            text("On"),
            sk_iced::components::ToggleSwitch::new(appearance, app.motion, true)
                .on_toggle(Message::SearchSubfolders),
        ]
        .spacing(24)
        .align_y(iced::Center);
        let pixels = final_ui_checks::render_view(
            container(controls).center(Fill).into(),
            &app,
            iced::Size::new(360, 100),
            None,
        );
        tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(360, 100).unwrap())
            .unwrap()
            .save_png(format!("target/neumorphic-switches-{label}.png"))
            .unwrap();
    }
}
