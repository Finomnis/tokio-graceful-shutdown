use bytemuck::NoUninit;

#[derive(Clone, Copy, Debug, Eq, PartialEq, NoUninit)]
#[repr(u8)]
pub enum ErrorAction {
    Forward,
    CatchAndLocalShutdown,
}
