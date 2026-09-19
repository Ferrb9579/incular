/// Platform adapters may translate these stable values to native IME hints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputType {
    #[default]
    Text,
    Multiline,
    Number,
    Phone,
    Datetime,
    EmailAddress,
    Url,
    VisiblePassword,
    Name,
    StreetAddress,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputAction {
    #[default]
    Unspecified,
    None,
    Done,
    Go,
    Search,
    Send,
    Next,
    Previous,
    Continue,
    Join,
    Route,
    EmergencyCall,
    Newline,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DayPeriod {
    #[default]
    Am,
    Pm,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeOfDayFormat {
    #[default]
    HH_colon_mm,
    HH_dot_mm,
    frenchCanadian,
    a_space_h_colon_mm,
    H_colon_mm,
    h_colon_mm_space_a,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HourFormat {
    HH,
    H,
    #[default]
    h,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimePickerEntryMode {
    #[default]
    Dial,
    Input,
    DialOnly,
    InputOnly,
}
