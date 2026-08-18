use leptos::prelude::Children;
use leptos::prelude::ClassAttribute;
use leptos::prelude::ElementChild;
use leptos::prelude::OnAttribute;
use leptos::*;

// Base classes that apply to all buttons
// We export this to allow `ButtonGroup` to overwrite these classes
pub const BASE_CLASSES: &str = r#"
  inline-flex items-center justify-center m-2
  rounded-md border-0 cursor-pointer
  font-mono font-medium leading-none
  shadow-sm
  transition-all duration-150
  select-none touch-manipulation
  whitespace-nowrap overflow-hidden
  hover:shadow-lg
  hover:-translate-y-0.5
  focus:shadow-xl
  focus:outline-none
  active:shadow-sm
  active:translate-y-0.5
"#;

// Define possible variants and sizes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Failure,
    Danger,
    Success,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonSize {
    Sm,
    Md,
    Lg,
    Xl,
}

impl Default for ButtonVariant {
    fn default() -> Self {
        Self::Secondary
    }
}

impl Default for ButtonSize {
    fn default() -> Self {
        Self::Sm
    }
}

impl ButtonVariant {
    fn classes(&self) -> &'static str {
        match self {
            Self::Primary => "bg-gradient-to-br from-blue-500 to-blue-600 text-white",
            Self::Secondary => "bg-gray-200 text-gray-900",
            Self::Failure => "bg-red-500 text-white hover:opacity-90",
            Self::Danger => "bg-red-500 text-white hover:opacity-90",
            Self::Success => "bg-green-500 text-white hover:opacity-90",
            Self::Warning => "bg-yellow-500 text-white hover:opacity-90",
        }
    }
}

impl ButtonSize {
    fn classes(&self) -> &'static str {
        match self {
            Self::Sm => "px-2 py-1 text-xs h-8",
            Self::Md => "px-4 py-2 text-sm h-10",
            Self::Lg => "px-4 py-2 text-base h-12",
            Self::Xl => "px-6 py-3 text-lg h-14",
        }
    }
}

#[component]
pub fn Button(
    children: Children,
    #[prop(optional)] variant: Option<ButtonVariant>,
    #[prop(optional)] size: Option<ButtonSize>,
    #[prop(optional)] class: Option<String>,
    #[prop(optional)] disabled: Option<bool>,
    #[prop(optional)] on_click: Option<Box<dyn Fn() + 'static>>,
) -> impl IntoView {
    let variant = variant.unwrap_or_default();
    let size = size.unwrap_or_default();
    let class = class.unwrap_or_default();
    let disabled = disabled.unwrap_or(false);

    // Combine all classes (simplified version of twMerge)
    let button_classes = format!(
        "{} {} {} {}",
        BASE_CLASSES.trim(),
        variant.classes(),
        size.classes(),
        class
    );

    view! {
        <button
            class=button_classes
            disabled=disabled
            on:click=move |_| {
                if let Some(ref handler) = on_click {
                    handler();
                }
            }
        >
            {children()}
        </button>
    }
}
