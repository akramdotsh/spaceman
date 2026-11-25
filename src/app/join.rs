use super::*;

impl<B: WorkspaceBackend> WorkspaceRuntime<B> {
    /// Nests the focused tile with the tile `Option+H/J/K/L` would focus in
    /// `direction`, in a split across that direction (issue 250). Only tiles on
    /// the focused workspace qualify: without one, the command does nothing
    /// rather than crossing displays or splitting the whole workspace.
    pub(super) fn join_focused(
        &mut self,
        direction: Direction,
        outputs: macos::DisplayOutputs,
        cancelled: &AtomicBool,
    ) -> Result<CycleStatus, String> {
        let (window, workspace) = self.focused_active_workspace(&outputs, cancelled, true)?;
        self.workspaces
            .deck(workspace)
            .require_split("joining tiles in")?;
        // Only this workspace's tiles qualify: join never reaches another
        // display, and `join_window` holds only this workspace's slots.
        let local_tiles = self
            .navigation_tiles(&outputs)
            .into_iter()
            .filter(|tile| tile.workspace == workspace)
            .collect::<Vec<_>>();
        let Some(target) = crate::navigation::target(&local_tiles, &window, direction) else {
            return Ok(self.status(0, &outputs));
        };
        let settings = self.layout_settings(output_for(&self.workspaces, workspace, &outputs));
        if !self.workspaces.engine_mut(workspace).join_window(
            &window,
            &target.window,
            direction,
            &settings,
        ) {
            return Ok(self.status(0, &outputs));
        }
        self.relayout_persist(workspace, &outputs, cancelled)
    }
}
