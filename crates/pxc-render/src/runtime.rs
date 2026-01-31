#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color(pub u32);

#[derive(Clone, Debug, Default)]
pub struct Gradient {
    pub keys: Vec<(f32, Color)>,
    pub interp: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub pixels_rgba8: Vec<u8>,
}
