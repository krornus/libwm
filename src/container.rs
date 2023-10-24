use xcb::x;

use crate::layout;
use crate::tree::Tree;
use crate::rect::Rect;
use crate::error::Error;
use crate::window::Window;
use crate::manager::{Connection, Event};


fn get_window_rect(conn: &xcb::Connection, window: x::Window) -> Result<Rect, Error> {
    let cookie = conn.send_request(&x::GetGeometry {
        drawable: x::Drawable::Window(window),
    });

    let reply = conn.wait_for_reply(cookie)?;

    Ok(Rect::new(reply.x(), reply.y(), reply.width(), reply.height()))
}


#[repr(transparent)]
pub struct Layout {
    inner: Box<dyn layout::Layout>
}

impl Layout {
    #[inline]
    pub fn new<L: layout::Layout + 'static>(layout: L) -> Self {
        Self {
            inner: Box::new(layout)
        }
    }
}

pub enum ContainerNode {
    Window(Window),
    Layout(Layout),
}

impl AsRef<Window> for ContainerNode {
    fn as_ref(&self) -> &Window {
        match self {
            ContainerNode::Window(x) => x,
            ContainerNode::Layout(_) => panic!("attempt to take non-window as ref"),
        }
    }
}

impl AsRef<Layout> for ContainerNode {
    fn as_ref(&self) -> &Layout {
        match self {
            ContainerNode::Layout(x) => x,
            ContainerNode::Window(_) => panic!("attempt to take non-layout as ref"),
        }
    }
}

impl AsMut<Window> for ContainerNode {
    fn as_mut(&mut self) -> &mut Window {
        match self {
            ContainerNode::Window(x) => x,
            ContainerNode::Layout(_) => panic!("attempt to take non-window as ref"),
        }
    }
}

impl AsMut<Layout> for ContainerNode {
    fn as_mut(&mut self) -> &mut Layout {
        match self {
            ContainerNode::Layout(x) => x,
            ContainerNode::Window(_) => panic!("attempt to take non-layout as ref"),
        }
    }
}

impl ContainerNode {
    #[inline]
    pub fn as_window_ref(&self) -> &Window {
        self.as_ref()
    }

    #[inline]
    pub fn as_layout_ref(&self) -> &Layout {
        self.as_ref()
    }

    #[inline]
    pub fn as_window_mut(&mut self) -> &mut Window {
        self.as_mut()
    }

    #[inline]
    pub fn as_layout_mut(&mut self) -> &mut Layout {
        self.as_mut()
    }
}

impl From<Window> for ContainerNode {
    fn from(window: Window) -> Self {
        ContainerNode::Window(window)
    }
}

impl<L: layout::Layout + 'static> From<L> for ContainerNode {
    fn from(layout: L) -> Self {
        ContainerNode::Layout(Layout::new(layout))
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ContainerId {
    id: usize,
}

pub struct Container {
    conn: Connection,
    tree: Tree<ContainerNode>,
}

impl Container {
    pub fn new(conn: Connection) -> Result<Self, Error> {

        let size = get_window_rect(conn.raw(), conn.root())?;
        let root = Window::new(conn.clone(), conn.root(), size, false, false);
        let node = ContainerNode::Window(root);

        Ok(Self {
            conn: conn,
            tree: Tree::new(node),
        })
    }

    pub fn insert<T: Into<ContainerNode>>(&mut self, parent: ContainerId, value: T) -> ContainerId {
        ContainerId {
            id: self.tree.insert(parent.id, value.into())
        }
    }

    pub fn from_window(&self, window: x::Window) -> Option<ContainerId> {
        let windows = self.tree.iter().filter_map(|(id, win)| {
            match &win.value {
                ContainerNode::Window(x) => Some((id, x.window())),
                ContainerNode::Layout(_) => None,
            }
        });

        for (id, x) in windows {
            if window == x {
                return Some(ContainerId { id })
            }
        }

        None
    }

    /// Same as arrange, but using index as the root
    pub fn arrange_at(&mut self, index: usize, size: Rect) -> Result<(), Error> {
        /* children indices must first be collected. layouts need to know
         * how many total windows they are dealing with. */
        let children: Vec<usize> = self.tree.children(index).collect();
        let count = children.len();

        let mut cells = Vec::with_capacity(count);

        let is_layout;
        let node = &mut self.tree[index];

        match &mut node.value {
            ContainerNode::Layout(layout) => {
                is_layout = true;

                for i in 0..count {
                    let cell = layout.inner.arrange(i, count, false, size);
                    cells.push(cell);
                }
            }
            _ => {
                is_layout = false;
            }
        }

        if is_layout {
            for (child, cell) in std::iter::zip(children.iter(), cells.into_iter()) {
                let node = &mut self.tree[*child];

                match &mut node.value {
                    ContainerNode::Window(window) => {
                        match cell {
                            layout::Cell::Hide => {
                                window.hide()?;
                            }
                            layout::Cell::Show(size) => {
                                window.resize(size)?;
                                window.show()?;
                            }
                            layout::Cell::Focus(size) => {
                                window.resize(size)?;
                                window.focus()?;
                                window.show()?;
                            }
                        }
                    }
                    ContainerNode::Layout(_) => {
                        match cell {
                            layout::Cell::Hide => {
                                self.hide(ContainerId { id: *child })?;
                            }
                            layout::Cell::Show(size) => {
                                self.arrange_at(*child, size)?;
                            }
                            layout::Cell::Focus(_) => {
                                unimplemented!();
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Arrange this container by resizing all sub-windows according to their parent layouts
    pub fn arrange(&mut self, scope: Rect) -> Result<(), Error> {
        self.arrange_at(self.tree.root(), scope)
    }

    pub fn show(&mut self, id: ContainerId) -> Result<(), Error> {
        let indices: Vec<_> = self.tree.iter_at(id.id).collect();

        for i in indices.into_iter() {
            match &mut self.tree[i].value {
                ContainerNode::Window(w) => { w.show()?; }
                ContainerNode::Layout(_) => {},
            }
        }

        Ok(())
    }

    pub fn hide(&mut self, id: ContainerId) -> Result<(), Error> {
        let indices: Vec<_> = self.tree.iter_at(id.id).collect();

        for i in indices.into_iter() {
            match &mut self.tree[i].value {
                ContainerNode::Window(w) => { w.hide()?; }
                ContainerNode::Layout(_) => {},
            }
        }

        Ok(())
    }

    pub fn create(&mut self, event: &x::CreateNotifyEvent) {
        let parent = self.from_window(event.parent())
            .unwrap_or(ContainerId { id: self.tree.root() });

        let size = Rect::new(event.x(), event.y(), event.width(), event.height());
        let win = Window::new(self.conn.clone(), event.window(), size, !event.override_redirect(), true);
        let id = self.insert(parent, win);

        self.conn.produce(Event::WindowCreate {
            window: id,
            x: event.x(),
            y: event.y(),
            width: event.width(),
            height: event.height(),
        });
    }

    pub fn configure(&mut self, event: &x::ConfigureRequestEvent) {
        match self.from_window(event.window()) {
            Some(id) => {
                self.conn.produce(Event::WindowResize {
                    window: id,
                    x: event.x(),
                    y: event.y(),
                    width: event.width(),
                    height: event.height(),
                });
            },
            None => {
                eprintln!("Unknown window configured!");
            }
        }
    }

    pub fn map(&mut self, event: &x::MapRequestEvent) {
        match self.from_window(event.window()) {
            Some(id) => {
                self.conn.produce(Event::WindowShow {
                    window: id
                });
            },
            None => {
                eprintln!("Unknown window mapped!");
            }
        }
    }
}

impl std::ops::Index<ContainerId> for Container {
    type Output = ContainerNode;

    fn index(&self, id: ContainerId) -> &Self::Output {
        &self.tree[id.id].value
    }
}

impl std::ops::IndexMut<ContainerId> for Container {
    fn index_mut(&mut self, id: ContainerId) -> &mut Self::Output {
        &mut self.tree[id.id].value
    }
}
