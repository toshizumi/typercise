use gloo_timers::callback::Interval;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Deserialize, Clone, Default, Debug)]
struct LiveStats {
    today: u64,
    #[serde(default)]
    corrections: u64,
    #[serde(default)]
    rework: f64,
    kcal: f64,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[allow(dead_code)]
struct TodayStats {
    total: u64,
    per_hour: [u64; 24],
    #[serde(default)]
    corrections: u64,
    #[serde(default)]
    rework: f64,
    kcal: f64,
    #[serde(default)]
    active_minutes: u32,
    #[serde(default)]
    avg_kpm: u32,
    #[serde(default)]
    peak_kpm: u32,
}

#[derive(Deserialize, Clone, Default, Debug)]
struct WeekdayStats {
    avg: [u64; 7],
}

#[derive(Deserialize, Clone, Default, Debug)]
struct WeekStats {
    total: u64,
    per_day: [u64; 7],
    start_date: String,
    kcal: f64,
}

#[derive(Deserialize, Clone, Default, Debug)]
struct MonthStats {
    total: u64,
    per_day: Vec<u64>,
    year: i32,
    month: u32,
    kcal: f64,
}

#[derive(Deserialize, Clone, Default, Debug)]
struct TotalStats {
    total: u64,
    since_ts: Option<i64>,
    kcal: f64,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
struct Settings {
    client_id: String,
    #[serde(default)]
    telemetry_enabled: bool,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    first_run_consent_shown: bool,
    #[serde(default)]
    last_uploaded_date: Option<String>,
}

#[derive(Deserialize, Clone, Default, Debug)]
struct WorldStats {
    #[serde(default)]
    participants_7d: u64,
    #[serde(default)]
    today: WorldToday,
    #[serde(default)]
    all_time: WorldAllTime,
    #[serde(default)]
    top_today_keys: Vec<RankKeys>,
    #[serde(default)]
    top_today_peak_kpm: Vec<RankPeak>,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[allow(dead_code)]
struct WorldToday {
    #[serde(default)]
    keys: u64,
    #[serde(default)]
    kcal: f64,
    #[serde(default)]
    active_users: u64,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[allow(dead_code)]
struct WorldAllTime {
    #[serde(default)]
    keys: u64,
    #[serde(default)]
    kcal: f64,
}

#[derive(Deserialize, Clone, Debug)]
struct RankKeys {
    nickname: String,
    keys: u64,
}

#[derive(Deserialize, Clone, Debug)]
struct RankPeak {
    nickname: String,
    peak_kpm: u32,
}

#[derive(Serialize)]
struct OffsetArg {
    offset: i32,
}

#[derive(Serialize)]
struct EnabledArg {
    enabled: bool,
}

#[derive(Serialize)]
struct NicknameArg {
    nickname: Option<String>,
}

#[derive(Serialize)]
struct ConsentArg {
    consented: bool,
}

#[derive(Serialize)]
struct EmptyArgs {}

async fn call<T: for<'de> Deserialize<'de>>(cmd: &str, args: JsValue) -> Option<T> {
    let v = invoke(cmd, args).await;
    serde_wasm_bindgen::from_value(v).ok()
}

fn empty_args() -> JsValue {
    serde_wasm_bindgen::to_value(&EmptyArgs {}).unwrap_or(JsValue::NULL)
}

fn offset_args(offset: i32) -> JsValue {
    serde_wasm_bindgen::to_value(&OffsetArg { offset }).unwrap_or(JsValue::NULL)
}

fn format_num(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Tab {
    Today,
    Weekday,
    Week,
    Month,
    Total,
    World,
}

#[component]
pub fn App() -> impl IntoView {
    let (accessibility_ok, set_acc_ok) = signal(true);
    let (tab, set_tab) = signal(Tab::Today);
    let (live, set_live) = signal::<LiveStats>(LiveStats::default());
    let (settings, set_settings) = signal::<Option<Settings>>(None);
    let (show_settings, set_show_settings) = signal(false);

    // Check accessibility permission + load settings on mount.
    Effect::new(move |_| {
        spawn_local(async move {
            if let Some(ok) = call::<bool>("check_accessibility", empty_args()).await {
                set_acc_ok.set(ok);
            }
            if let Some(s) = call::<Settings>("get_settings", empty_args()).await {
                set_settings.set(Some(s));
            }
        });
    });

    // Poll live stats every 1s.
    Effect::new(move |_| {
        let interval = Interval::new(1000, move || {
            spawn_local(async move {
                if let Some(v) = call::<LiveStats>("get_live", empty_args()).await {
                    set_live.set(v);
                }
            });
        });
        interval.forget();
    });

    let request_perm = move |_| {
        spawn_local(async move {
            let _ = call::<bool>("request_accessibility", empty_args()).await;
        });
    };

    let mark_consent = move |consented: bool| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&ConsentArg { consented })
                .unwrap_or(JsValue::NULL);
            if let Some(s) = call::<Settings>("mark_consent_shown", args).await {
                set_settings.set(Some(s));
            }
        });
    };

    let need_consent = move || matches!(settings.get(), Some(s) if !s.first_run_consent_shown);

    view! {
        <div class="shell">
            <header>
                <button class="gear" title="設定" on:click=move |_| set_show_settings.update(|v| *v = !*v)>"⚙"</button>
                <div class="counter-label">"今日の打鍵数"</div>
                <div class="counter">{move || format_num(live.get().today)}</div>
                <div class="kcal">
                    {move || {
                        let l = live.get();
                        let rework_text = if l.today + l.corrections == 0 {
                            "訂正率 —".to_string()
                        } else {
                            format!("訂正率 {:.1}%", l.rework * 100.0)
                        };
                        format!("≈ {:.2} kcal（参考値） · {}", l.kcal, rework_text)
                    }}
                </div>
            </header>

            <Show when=move || !accessibility_ok.get() fallback=|| ()>
                <div class="banner">
                    <div>"アクセシビリティ権限が未許可のため、キー入力を検知できません。"</div>
                    <button on:click=request_perm>"システム設定を開く"</button>
                </div>
            </Show>

            <TabBar current=tab set_tab=set_tab/>

            <div class="content">
                {move || match tab.get() {
                    Tab::Today => view! { <TodayView/> }.into_any(),
                    Tab::Weekday => view! { <WeekdayView/> }.into_any(),
                    Tab::Week => view! { <WeekView/> }.into_any(),
                    Tab::Month => view! { <MonthView/> }.into_any(),
                    Tab::Total => view! { <TotalView/> }.into_any(),
                    Tab::World => view! { <WorldView/> }.into_any(),
                }}
            </div>

            <div class="footer">"Typercise · カロリーは参考値です"</div>

            <Show when=move || show_settings.get() fallback=|| ()>
                <SettingsPanel
                    settings=settings
                    set_settings=set_settings
                    on_close=Box::new(move || set_show_settings.set(false))
                />
            </Show>

            <Show when=need_consent fallback=|| ()>
                <ConsentModal on_decide=Box::new(mark_consent)/>
            </Show>
        </div>
    }
}

#[component]
fn TabBar(current: ReadSignal<Tab>, set_tab: WriteSignal<Tab>) -> impl IntoView {
    let tabs = [
        (Tab::Today, "今日"),
        (Tab::Weekday, "曜日別"),
        (Tab::Week, "週"),
        (Tab::Month, "月"),
        (Tab::Total, "累計"),
        (Tab::World, "世界"),
    ];
    view! {
        <div class="tabs">
            {tabs.iter().map(|(t, label)| {
                let t = *t;
                let label = *label;
                view! {
                    <div
                        class=move || if current.get() == t { "tab active" } else { "tab" }
                        on:click=move |_| set_tab.set(t)
                    >{label}</div>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn TodayView() -> impl IntoView {
    let (data, set_data) = signal::<Option<TodayStats>>(None);
    Effect::new(move |_| {
        spawn_local(async move {
            set_data.set(call::<TodayStats>("get_today", empty_args()).await);
        });
    });

    view! {
        <div>
            <div class="stat-row">
                <span class="stat-label">"合計"</span>
                <span class="stat-value">{move || data.get().map(|d| format_num(d.total)).unwrap_or_else(|| "—".into())}</span>
            </div>
            <BarChart
                values=Signal::derive(move || data.get().map(|d| d.per_hour.to_vec()).unwrap_or_default())
                labels=hour_labels()
            />
            <div class="legend"><span>"0:00"</span><span>"12:00"</span><span>"23:00"</span></div>

            <div class="stat-row" style="margin-top:14px;">
                <span class="stat-label">"活動時間"</span>
                <span class="stat-value">{move || data.get().map(|d| format!("{} 分", d.active_minutes)).unwrap_or_else(|| "—".into())}</span>
            </div>
            <div class="stat-row">
                <span class="stat-label">"平均速度"</span>
                <span class="stat-value">
                    {move || data.get().map(|d| {
                        if d.avg_kpm == 0 { "—".into() }
                        else { format!("{} kpm（≈ {} wpm）", d.avg_kpm, d.avg_kpm / 5) }
                    }).unwrap_or_else(|| "—".into())}
                </span>
            </div>
            <div class="stat-row">
                <span class="stat-label">"ピーク速度"</span>
                <span class="stat-value">
                    {move || data.get().map(|d| {
                        if d.peak_kpm == 0 { "—".into() }
                        else { format!("{} kpm（≈ {} wpm）", d.peak_kpm, d.peak_kpm / 5) }
                    }).unwrap_or_else(|| "—".into())}
                </span>
            </div>
        </div>
    }
}

fn hour_labels() -> Vec<String> {
    (0..24).map(|h| format!("{h}")).collect()
}

fn weekday_labels() -> Vec<String> {
    vec!["月", "火", "水", "木", "金", "土", "日"]
        .into_iter()
        .map(String::from)
        .collect()
}

#[component]
fn WeekdayView() -> impl IntoView {
    let (data, set_data) = signal::<Option<WeekdayStats>>(None);
    Effect::new(move |_| {
        spawn_local(async move {
            set_data.set(call::<WeekdayStats>("get_weekday_avg", empty_args()).await);
        });
    });

    view! {
        <div>
            <div class="stat-row">
                <span class="stat-label">"過去8週の曜日別平均"</span>
                <span class="stat-value">{move || data.get().map(|d| format_num(d.avg.iter().sum::<u64>() / 7)).unwrap_or_else(|| "—".into())}</span>
            </div>
            <BarChart
                values=Signal::derive(move || data.get().map(|d| d.avg.to_vec()).unwrap_or_default())
                labels=weekday_labels()
            />
        </div>
    }
}

#[component]
fn WeekView() -> impl IntoView {
    let (offset, set_offset) = signal(0i32);
    let (data, set_data) = signal::<Option<WeekStats>>(None);

    Effect::new(move |_| {
        let o = offset.get();
        spawn_local(async move {
            set_data.set(call::<WeekStats>("get_week", offset_args(o)).await);
        });
    });

    view! {
        <div>
            <div class="nav-row">
                <button class="nav-btn" on:click=move |_| set_offset.update(|o| *o -= 1)>"←"</button>
                <div class="title">{move || data.get().map(|d| d.start_date.clone()).unwrap_or_default()}</div>
                <button
                    class="nav-btn"
                    disabled=move || offset.get() >= 0
                    on:click=move |_| set_offset.update(|o| if *o < 0 { *o += 1 })
                >"→"</button>
            </div>
            <div class="stat-row">
                <span class="stat-label">"週合計"</span>
                <span class="stat-value">{move || data.get().map(|d| format_num(d.total)).unwrap_or_else(|| "—".into())}</span>
            </div>
            <BarChart
                values=Signal::derive(move || data.get().map(|d| d.per_day.to_vec()).unwrap_or_default())
                labels=weekday_labels()
            />
            <div class="stat-row" style="margin-top:10px;">
                <span class="stat-label">"推定カロリー"</span>
                <span class="stat-value">{move || data.get().map(|d| format!("{:.1} kcal", d.kcal)).unwrap_or_else(|| "—".into())}</span>
            </div>
        </div>
    }
}

#[component]
fn MonthView() -> impl IntoView {
    let (offset, set_offset) = signal(0i32);
    let (data, set_data) = signal::<Option<MonthStats>>(None);

    Effect::new(move |_| {
        let o = offset.get();
        spawn_local(async move {
            set_data.set(call::<MonthStats>("get_month", offset_args(o)).await);
        });
    });

    view! {
        <div>
            <div class="nav-row">
                <button class="nav-btn" on:click=move |_| set_offset.update(|o| *o -= 1)>"←"</button>
                <div class="title">{move || data.get().map(|d| format!("{}年 {}月", d.year, d.month)).unwrap_or_default()}</div>
                <button
                    class="nav-btn"
                    disabled=move || offset.get() >= 0
                    on:click=move |_| set_offset.update(|o| if *o < 0 { *o += 1 })
                >"→"</button>
            </div>
            <div class="stat-row">
                <span class="stat-label">"月合計"</span>
                <span class="stat-value">{move || data.get().map(|d| format_num(d.total)).unwrap_or_else(|| "—".into())}</span>
            </div>
            <BarChart
                values=Signal::derive(move || data.get().map(|d| d.per_day.clone()).unwrap_or_default())
                labels=day_labels_for(Signal::derive(move || data.get().map(|d| d.per_day.len()).unwrap_or(0)))
            />
            <div class="stat-row" style="margin-top:10px;">
                <span class="stat-label">"推定カロリー"</span>
                <span class="stat-value">{move || data.get().map(|d| format!("{:.1} kcal", d.kcal)).unwrap_or_else(|| "—".into())}</span>
            </div>
        </div>
    }
}

fn day_labels_for(_n: Signal<usize>) -> Vec<String> {
    (1..=31).map(|d| format!("{d}")).collect()
}

#[component]
fn TotalView() -> impl IntoView {
    let (data, set_data) = signal::<Option<TotalStats>>(None);
    Effect::new(move |_| {
        spawn_local(async move {
            set_data.set(call::<TotalStats>("get_total", empty_args()).await);
        });
    });

    let since_text = move || {
        data.get()
            .and_then(|d| d.since_ts)
            .map(|ts| {
                let date = js_sys::Date::new(&JsValue::from_f64((ts as f64) * 1000.0));
                let y = date.get_full_year();
                let m = date.get_month() + 1;
                let d = date.get_date();
                format!("{y}年{m}月{d}日から")
            })
            .unwrap_or_else(|| "—".into())
    };

    view! {
        <div>
            <div class="stat-row">
                <span class="stat-label">"これまでの合計"</span>
                <span class="stat-value">{move || data.get().map(|d| format_num(d.total)).unwrap_or_else(|| "—".into())}</span>
            </div>
            <div class="stat-row">
                <span class="stat-label">"推定カロリー"</span>
                <span class="stat-value">{move || data.get().map(|d| format!("{:.1} kcal", d.kcal)).unwrap_or_else(|| "—".into())}</span>
            </div>
            <div class="stat-row">
                <span class="stat-label">"記録開始"</span>
                <span class="stat-value" style="font-size:12px;">{since_text}</span>
            </div>
        </div>
    }
}

#[component]
fn BarChart(
    values: Signal<Vec<u64>>,
    labels: Vec<String>,
) -> impl IntoView {
    view! {
        <svg class="chart" viewBox="0 0 280 140" preserveAspectRatio="none">
            {move || {
                let vs = values.get();
                if vs.is_empty() { return vec![]; }
                let n = vs.len().max(1);
                let max = *vs.iter().max().unwrap_or(&1).max(&1);
                let w = 280.0 / n as f64;
                let bar_w = (w * 0.7).max(1.0);
                let off = (w - bar_w) / 2.0;
                vs.iter().enumerate().map(|(i, v)| {
                    let x = i as f64 * w + off;
                    let h = (*v as f64 / max as f64) * 120.0;
                    let y = 130.0 - h;
                    view! {
                        <g>
                            <rect class="bar-bg" x=x y="10" width=bar_w height="120" rx="1"/>
                            <rect class="bar" x=x y=y width=bar_w height=h rx="1"/>
                        </g>
                    }.into_any()
                }).collect::<Vec<_>>()
            }}
            {
                let labels = labels.clone();
                move || {
                    let vs = values.get();
                    let n = vs.len().max(1);
                    let w = 280.0 / n as f64;
                    let step = (n / 6).max(1);
                    (0..n).step_by(step).filter_map(|i| {
                        labels.get(i).map(|l| {
                            view! {
                                <text class="label" x={i as f64 * w + w / 2.0} y="140" text-anchor="middle">{l.clone()}</text>
                            }.into_any()
                        })
                    }).collect::<Vec<_>>()
                }
            }
        </svg>
    }
}

#[component]
fn WorldView() -> impl IntoView {
    let (data, set_data) = signal::<Option<WorldStats>>(None);
    let (loading, set_loading) = signal(true);
    let (err, set_err) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        spawn_local(async move {
            set_loading.set(true);
            match call::<WorldStats>("get_world_stats", empty_args()).await {
                Some(v) => {
                    set_data.set(Some(v));
                    set_err.set(None);
                }
                None => set_err.set(Some("読み込みに失敗しました".into())),
            }
            set_loading.set(false);
        });
    });

    view! {
        <div>
            <Show when=move || loading.get() && data.get().is_none() fallback=|| ()>
                <div class="muted-line">"読み込み中..."</div>
            </Show>
            <Show when=move || err.get().is_some() fallback=|| ()>
                <div class="banner">{move || err.get().unwrap_or_default()}</div>
            </Show>
            {move || data.get().map(|w| {
                let participants = w.participants_7d;
                let today_keys = w.today.keys;
                let today_kcal = w.today.kcal;
                let all_keys = w.all_time.keys;
                let top_keys = w.top_today_keys.clone();
                let top_peak = w.top_today_peak_kpm.clone();
                let keys_empty = top_keys.is_empty();
                let peak_empty = top_peak.is_empty();
                let keys_items = top_keys.into_iter().map(|r| {
                    view! {
                        <li><span class="rank-name">{r.nickname}</span><span class="rank-val">{format_num(r.keys)}</span></li>
                    }
                }).collect::<Vec<_>>();
                let peak_items = top_peak.into_iter().map(|r| {
                    view! {
                        <li><span class="rank-name">{r.nickname}</span><span class="rank-val">{format!("{} kpm", r.peak_kpm)}</span></li>
                    }
                }).collect::<Vec<_>>();
                view! {
                    <div>
                        <div class="stat-row">
                            <span class="stat-label">"参加者（7日)"</span>
                            <span class="stat-value">{format_num(participants)}</span>
                        </div>
                        <div class="stat-row">
                            <span class="stat-label">"今日 みんなで"</span>
                            <span class="stat-value">{format!("{} 打鍵", format_num(today_keys))}</span>
                        </div>
                        <div class="stat-row">
                            <span class="stat-label">"今日のカロリー"</span>
                            <span class="stat-value">{format!("{:.1} kcal", today_kcal)}</span>
                        </div>
                        <div class="stat-row">
                            <span class="stat-label">"累計"</span>
                            <span class="stat-value">{format!("{} 打鍵", format_num(all_keys))}</span>
                        </div>

                        <div class="rank-title">"今日の打鍵 TOP10"</div>
                        {if keys_empty {
                            view! { <div class="muted-line">"まだ誰もランクインしていません"</div> }.into_any()
                        } else {
                            view! { <ol class="ranking">{keys_items}</ol> }.into_any()
                        }}

                        <div class="rank-title">"今日のピーク kpm TOP10"</div>
                        {if peak_empty {
                            view! { <div class="muted-line">"まだ誰もランクインしていません"</div> }.into_any()
                        } else {
                            view! { <ol class="ranking">{peak_items}</ol> }.into_any()
                        }}
                    </div>
                }
            })}
        </div>
    }
}

#[component]
fn SettingsPanel(
    settings: ReadSignal<Option<Settings>>,
    set_settings: WriteSignal<Option<Settings>>,
    on_close: Box<dyn Fn() + 'static>,
) -> impl IntoView {
    let on_close = std::rc::Rc::new(on_close);
    let close_backdrop = {
        let oc = std::rc::Rc::clone(&on_close);
        move |_| (oc)()
    };
    let close_btn = {
        let oc = std::rc::Rc::clone(&on_close);
        move |_| (oc)()
    };
    let (nick_draft, set_nick_draft) = signal(String::new());

    // Sync draft with current nickname
    Effect::new(move |_| {
        if let Some(s) = settings.get() {
            set_nick_draft.set(s.nickname.unwrap_or_default());
        }
    });

    let toggle_telemetry = move |ev: web_sys::Event| {
        let target = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
        if let Some(input) = target {
            let enabled = input.checked();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&EnabledArg { enabled })
                    .unwrap_or(JsValue::NULL);
                if let Some(s) = call::<Settings>("set_telemetry_enabled", args).await {
                    set_settings.set(Some(s));
                }
            });
        }
    };

    let save_nick = move |_| {
        let nickname = nick_draft.get();
        let nickname = if nickname.trim().is_empty() { None } else { Some(nickname) };
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&NicknameArg { nickname })
                .unwrap_or(JsValue::NULL);
            if let Some(s) = call::<Settings>("set_nickname", args).await {
                set_settings.set(Some(s));
            }
        });
    };

    view! {
        <div class="modal-backdrop" on:click=close_backdrop>
            <div class="modal" on:click=|ev| ev.stop_propagation()>
                <div class="modal-title">"設定"</div>

                <label class="setting-row">
                    <span>"世界の Typercizer に参加（匿名で集計値を送信）"</span>
                    <input
                        type="checkbox"
                        prop:checked=move || settings.get().map(|s| s.telemetry_enabled).unwrap_or(false)
                        on:change=toggle_telemetry
                    />
                </label>

                <div class="setting-row col">
                    <label>"ランキング用ニックネーム（任意、32文字以内）"</label>
                    <div class="nick-row">
                        <input
                            type="text"
                            class="nick-input"
                            placeholder="未設定はランキング非表示"
                            maxlength="32"
                            prop:value=move || nick_draft.get()
                            on:input=move |ev| set_nick_draft.set(event_target_value(&ev))
                        />
                        <button class="save-btn" on:click=save_nick>"保存"</button>
                    </div>
                </div>

                <div class="privacy-note">
                    "送信されるのは打鍵数などの集計値のみ。キーの内容は一切送信しません。"
                    <br/>
                    <a href="https://toshizumi.github.io/typercise/privacy.html" target="_blank">"プライバシー詳細"</a>
                </div>

                <button class="modal-close" on:click=close_btn>"閉じる"</button>
            </div>
        </div>
    }
}

#[component]
fn ConsentModal(on_decide: Box<dyn Fn(bool) + 'static>) -> impl IntoView {
    let on_decide = std::rc::Rc::new(on_decide);
    let yes = {
        let on_decide = std::rc::Rc::clone(&on_decide);
        move |_| (on_decide)(true)
    };
    let no = {
        let on_decide = std::rc::Rc::clone(&on_decide);
        move |_| (on_decide)(false)
    };
    view! {
        <div class="modal-backdrop">
            <div class="modal">
                <div class="modal-title">"世界の Typercizer に参加？"</div>
                <p class="modal-body">
                    "Typercise は希望者の" <strong>"匿名・集計打鍵数"</strong> "を1日1回サーバへ送信し、世界の Typercizer ランキングに反映します。"
                    <br/><br/>
                    "送るもの: 打鍵数、訂正数、kcal、速度、活動時間、任意のニックネーム"
                    <br/>
                    "送らないもの: " <strong>"キーの内容、入力先アプリ、ファイル、Apple ID 等"</strong>
                    <br/><br/>
                    "あとから設定でいつでも止められます。"
                </p>
                <div class="modal-actions">
                    <button class="primary" on:click=yes>"同意して始める"</button>
                    <button on:click=no>"送信せずに使う"</button>
                </div>
                <a class="privacy-link" href="https://toshizumi.github.io/typercise/privacy.html" target="_blank">"プライバシー詳細"</a>
            </div>
        </div>
    }
}
