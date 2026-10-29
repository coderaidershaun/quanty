//! The widgets that take an action or a value from the person: the button, the button that copies
//! a text, the text box, the dropdown, the slider, the step buttons, the rows of a list, the strip
//! of tabs and the bar of views.

mod button;
mod copy;
mod field;
mod list_row;
mod nav_bar;
mod tabs;

pub use button::Button;
pub use copy::CopyButton;
pub use field::{
    Dropdown, Slider, Step, Stepper, TextInput, TextInputResponse, paint_caret, style_list_rows,
};
pub use list_row::ListRow;
pub use nav_bar::{NavBar, NavItem};
pub use tabs::{Tab, TabStrip};
