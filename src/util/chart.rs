use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::*;
use web_sys::MouseEvent;

#[component]
pub fn LineChart(data: Vec<(f64, f64)>) -> impl IntoView {
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
    let min_y = min_y - y_padding;
    let max_y = max_y + y_padding;

    // Mouse tracking state
    let (mouse_x_percent, set_mouse_x_percent) = signal(0.0);
    let (is_hovering, set_is_hovering) = signal(false);

    // Convert mouse percentage to data x coordinate
    let mouse_to_data_x = move |percent: f64| min_x + (percent / 100.0) * (max_x - min_x);

    // Convert data coordinates to SVG coordinates (0-100 range)
    let data_to_svg_x = move |x: f64| ((x - min_x) / (max_x - min_x)) * 100.0;
    let data_to_svg_y = move |y: f64| (1.0 - (y - min_y) / (max_y - min_y)) * 100.0;

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
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            style="display: block; cursor: crosshair;"
            on:mousemove=on_mouse_move
            on:mouseenter=on_mouse_enter
            on:mouseleave=on_mouse_leave
        >
            // Background (transparent)
            <rect x="0" y="0" width="100" height="100" fill="transparent" stroke="none" />

            // Horizontal grid lines
            {
                let num_lines = 5;
                (0..=num_lines)
                    .map(|i| {
                        let y_percent = (i as f64 / num_lines as f64) * 100.0;
                        view! {
                            <line
                                x1="0"
                                y1=y_percent
                                x2="100"
                                y2=y_percent
                                stroke="#e5e5e5"
                                stroke-width="1.2"
                                vector-effect="non-scaling-stroke"
                            />
                        }
                    })
                    .collect_view()
            }

            // Price line - continuous and smooth
            <path
                d=path_data
                fill="none"
                stroke="#2563eb"
                stroke-width="0.5"
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
                    let base_radius = 0.8;
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
                                fill="#2563eb"
                                stroke="white"
                                stroke-width="0.3"
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
