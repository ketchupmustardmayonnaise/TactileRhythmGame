use crate::components::virtual_keypad::VirtualKeypad;
use leptos::html::Canvas;
use leptos::prelude::*;

#[component]
pub fn BrailleDisplay(
    node_ref: NodeRef<Canvas>,
    width: f64,
    height: f64,
    current_applet: ReadSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="flex-1 flex flex-col items-center bg-gray-200 overflow-y-auto p-8">
            <VirtualKeypad current_applet=current_applet>
                <canvas
                    node_ref=node_ref
                    width=width * crate::display::PIXEL_SCALE
                    height=height * crate::display::PIXEL_SCALE
                    class="block border border-black bg-black shadow-lg"
                />
            </VirtualKeypad>
        </div>
    }
}
