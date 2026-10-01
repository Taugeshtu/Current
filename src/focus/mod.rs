pub mod niri;

#[derive(Debug, Clone)]
pub struct FocusPoint {
    pub window_id: u64,
    pub pid: Option<u32>,
    pub app_id: Option<String>,
    pub title: Option<String>,
}

pub async fn get_focus() -> Option<FocusPoint> {
    niri::get_focus().await
}

pub async fn get_active_window_ids() -> Option<std::collections::HashSet<u64>> {
    niri::get_active_window_ids().await
}
