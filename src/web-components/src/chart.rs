use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::*;
use web_sys::MouseEvent;

#[component]
pub fn Chart(data: Vec<(f64, f64)>) -> impl IntoView {
    // Find data bounds
    let min_x = data.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let max_x = data
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = data.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let max_y = data
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max);

    // Add some padding to y range
    let y_padding = (max_y - min_y) * 0.1;
    let padded_min_y = min_y - y_padding;
    let padded_max_y = max_y + y_padding;

    // Get nice tick positions for the padded range
    let tick_positions = get_nice_ticks(padded_min_y, padded_max_y);

    // Mouse tracking state
    let (mouse_x_percent, set_mouse_x_percent) = signal(0.0);
    let (is_hovering, set_is_hovering) = signal(false);

    // Convert mouse percentage to data x coordinate
    let mouse_to_data_x = move |percent: f64| min_x + (percent / 100.0) * (max_x - min_x);

    // Convert data coordinates to SVG coordinates (0-100 range)
    let data_to_svg_x = move |x: f64| ((x - min_x) / (max_x - min_x)) * 100.0;
    let data_to_svg_y =
        move |y: f64| (1.0 - (y - padded_min_y) / (padded_max_y - padded_min_y)) * 100.0;

    // Find closest data point to mouse
    let data_clone = data.clone();
    let closest_point = Memo::new(move |_| {
        if !is_hovering.get() {
            return None;
        }

        let target_x = mouse_to_data_x(mouse_x_percent.get());
        data_clone
            .iter()
            .min_by(|(x1, _), (x2, _)| {
                (target_x - x1)
                    .abs()
                    .partial_cmp(&(target_x - x2).abs())
                    .unwrap()
            })
            .copied()
    });

    // Generate path data for the continuous line
    let path_data = data
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let svg_x = data_to_svg_x(*x);
            let svg_y = data_to_svg_y(*y);

            if i == 0 {
                format!("M {:.2} {:.2}", svg_x, svg_y)
            } else {
                format!("L {:.2} {:.2}", svg_x, svg_y)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Mouse event handlers
    let (container_aspect, set_container_aspect) = signal(1.0);

    let on_mouse_move = move |event: MouseEvent| {
        let svg = event.current_target().unwrap();
        let svg_element = svg.dyn_into::<web_sys::Element>().unwrap();
        let rect = svg_element.get_bounding_client_rect();

        // Update aspect ratio for ellipse calculation
        let aspect = rect.width() / rect.height();
        set_container_aspect.set(aspect);

        let x_percent = (event.client_x() as f64 - rect.left()) / rect.width() * 100.0;
        set_mouse_x_percent.set(x_percent);
    };

    let on_mouse_enter = move |_| set_is_hovering.set(true);
    let on_mouse_leave = move |_| set_is_hovering.set(false);

    view! {
        <svg
            width="100%"
            height="100%"
            viewBox="0 0 110 105"
            preserveAspectRatio="none"
            style="display: block; cursor: crosshair;"
            on:mouseenter=on_mouse_enter
            on:mouseleave=on_mouse_leave
        >
            // Background (transparent)
            <rect
                x="0"
                y="0"
                width="100"
                height="100"
                fill="transparent"
                stroke="none"
                on:mousemove=on_mouse_move
            />

            // Horizontal grid lines using nice ticks
            {
                tick_positions
                    .iter()
                    .map(|&tick_value| {
                        let y_percent = data_to_svg_y(tick_value);
                        view! {
                            <line
                                x1="0"
                                y1=y_percent
                                x2="100"
                                y2=y_percent
                                stroke="var(--chart-base)"
                                stroke-width="0.8"
                                vector-effect="non-scaling-stroke"
                            />
                            <text
                                x="102"
                                y=y_percent
                                text-anchor="start"
                                dominant-baseline="middle"
                                font-size="3"
                                fill="var(--chart-base)"
                                vector-effect="non-scaling-stroke"
                            >
                                {format!("{:.1}", tick_value)}
                            </text>
                        }
                    })
                    .collect_view()
            }

            // Price line - continuous and smooth
            <path
                d=path_data
                fill="none"
                stroke="var(--chart-accent)"
                stroke-width="2.0"
                stroke-linecap="round"
                stroke-linejoin="round"
                vector-effect="non-scaling-stroke"
            />

            // Crosshair - vertical line
            {move || {
                if let Some((x, _)) = closest_point.get() {
                    let snapped_x = data_to_svg_x(x);
                    Some(
                        view! {
                            <line
                                x1=snapped_x
                                y1="0"
                                x2=snapped_x
                                y2="100"
                                stroke="#999"
                                stroke-width="0.3"
                                stroke-dasharray="1,1"
                                vector-effect="non-scaling-stroke"
                            />
                        },
                    )
                } else {
                    None
                }
            }}

            // Highlighted point
            {move || {
                if let Some((x, y)) = closest_point.get() {
                    let svg_x = data_to_svg_x(x);
                    let svg_y = data_to_svg_y(y);
                    let base_radius = 0.5;
                    let aspect = container_aspect.get();
                    let rx = base_radius / aspect;
                    let ry = base_radius;
                    Some(

                        // Calculate proper radii to make a circle regardless of aspect ratio
                        // Compensate for horizontal stretching

                        view! {
                            <ellipse
                                cx=svg_x
                                cy=svg_y
                                rx=rx
                                ry=ry
                                fill="var(--chart-accent)"
                                vector-effect="non-scaling-stroke"
                            />
                        },
                    )
                } else {
                    None
                }
            }}
        </svg>
    }
}

const TARGET_TICKS: usize = 7;

/// Returns a list of "nice" tick mark positions for the given data range
fn get_nice_ticks(lo: f64, hi: f64) -> Vec<f64> {
    if lo >= hi {
        return vec![lo];
    }

    let range = hi - lo;
    let raw_step = range / (TARGET_TICKS - 1) as f64;

    // Find the magnitude (power of 10)
    let magnitude = 10_f64.powf(raw_step.log10().floor());

    // Normalize the step to be between 1 and 10
    let normalized_step = raw_step / magnitude;

    // Round to nearest "nice" number: 1, 2, 5, or 10
    let nice_step = if normalized_step <= 1.0 {
        1.0
    } else if normalized_step <= 2.0 {
        2.0
    } else if normalized_step <= 5.0 {
        5.0
    } else {
        10.0
    };

    let step = nice_step * magnitude;

    // Find nice start and end points
    let nice_start = (lo / step).floor() * step;
    let nice_end = (hi / step).ceil() * step;

    // Generate ticks
    let mut ticks = Vec::new();
    let mut current = nice_start;

    while current <= nice_end + f64::EPSILON {
        ticks.push(current);
        current += step;
    }

    ticks
}
