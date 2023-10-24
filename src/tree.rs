use slab;

pub struct TreeNode<T> {
    pub value: T,
    index: usize,
    parent: Option<usize>,
    first: Option<usize>,
    last: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
}

impl<T> std::fmt::Debug for TreeNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeNode<T>")
         .field("index", &self.index)
         .field("parent", &self.parent)
         .field("first", &self.first)
         .field("last", &self.last)
         .field("left", &self.left)
         .field("right", &self.right)
         .finish()
    }
}

impl<T> TreeNode<T> {
    fn new(index: usize, value: T) -> Self {
        TreeNode {
            value: value,
            index: index,
            parent: None,
            left: None,
            right: None,
            first: None,
            last: None,
        }
    }
}

impl<T> TreeNode<T> {
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }

    #[inline]
    pub fn previous_sibling(&self) -> Option<usize> {
        self.left
    }

    #[inline]
    pub fn next_sibling(&self) -> Option<usize> {
        self.right
    }

    #[inline]
    pub fn child(&self) -> Option<usize> {
        self.first
    }
}

pub struct Tree<T> {
    root: usize,
    slab: slab::Slab<TreeNode<T>>,
}

impl<T> Tree<T> {
    pub fn new(root: T) -> Self {
        let mut slab = slab::Slab::new();

        let index = slab.vacant_key();
        let node = TreeNode::new(index, root);
        slab.insert(node);

        Tree {
            root: index,
            slab: slab,
        }
    }

    pub fn root(&self) -> usize {
        self.root
    }

    /// Create and insert an unattached tree node
    fn orphan(&mut self, value: T) -> usize {
        let index = self.slab.vacant_key();
        let node = TreeNode::new(index, value);
        self.slab.insert(node)
    }

    /// Attach an orphan() to the tree at index
    fn adopt(&mut self, parent: usize, orphan: usize) {
        /* set the parent index in the new child */
        let parent_node = &mut self[parent];
        let sibling = parent_node.last.replace(orphan);

        /* replace the index of the child with the new key,
         * and set the child's sibing to the new key. */
        match sibling {
            Some(i) => {
                self[i].right = Some(orphan);
            }
            _ => {
                /* no children - set first and last */
                parent_node.first = Some(orphan);
            }
        }

        let mut node = &mut self[orphan];
        node.parent = Some(parent);
        node.left = sibling;

    }

    /// Insert value into tree with given parent
    pub fn insert(&mut self, parent: usize, value: T) -> usize {
        let orphan = self.orphan(value);
        self.adopt(parent, orphan);

        orphan
    }

    /// Remove a sub-tree from one tree and graft it into another
    pub fn graft(&mut self, other: &mut Tree<T>, from: usize, to: usize) {
        /* not the fastest way to do this, but the easiest to read */
        let children: Vec<_> = other.children(from).collect();
        let node = other.slab.remove(from);
        let index = self.insert(to, node.value);

        for child in children.into_iter().rev() {
            self.graft(other, child, index);
        }
    }

    /// Discard a node and its children from a tree with no further processing
    /// You probably want the recursive function prune(), found below
    fn discard(&mut self, index: usize) -> TreeNode<T> {
        let children: Vec<_> = self.children(index).collect();

        let root = self.slab.remove(index);

        for child in children.into_iter() {
            self.discard(child);
        }

        root
    }

    /// Extract a node from the tree and return its value, non-recursively
    pub fn extract(&mut self, index: usize) -> T {
        let root = self.slab.remove(index);

        /* check the left sibling. if there isn't one, we are the first child in
         * parent. otherwise, update the right sibling. */
        match root.left {
            Some(i) => {
                /* we have a sibling to the left. give it our right value */
                self[i].right = root.right;
            },
            None => {
                /* we are the first child for our parent.
                 * update parent.first to have our right value */
                match root.parent {
                    Some(j) => {
                        self[j].first = root.right;
                    },
                    None => {
                    }
                }
            },
        }

        /* check the right sibling. if there isn't one, we are the last child in
         * parent. otherwise, update the left sibling. */
        match root.right {
            Some(i) => {
                /* we have a sibling to the right. give it our left value */
                self[i].left = root.left;
            },
            None => {
                /* we are the last child for our parent.
                 * update parent.last to have our left value */
                match root.parent {
                    Some(j) => {
                        self[j].last = root.left;
                    },
                    None => {
                    }
                }
            },
        }

        root.value
    }

    /// Extract a node from the tree and return its value, recursively discarding children
    pub fn prune(&mut self, index: usize) -> T {
        let children: Vec<_> = self.children(index).collect();

        for child in children.into_iter() {
            self.discard(child);
        }

        self.extract(index)
    }

    /// Remove a node from the tree, returning a new tree with root at node
    pub fn remove(&mut self, index: usize) -> Tree<T> {
        let children: Vec<_> = self.children(index).collect();

        let root = self.slab.remove(index);
        let mut tree = Tree::new(root.value);

        for child in children.into_iter().rev() {
            tree.graft(self, child, tree.root);
        }

        tree
    }
}

impl<T> Tree<T> {
    pub fn children<'a>(&'a self, index: usize) -> Children<'a, T> {
        let child = self[index].first;

        Children {
            tree: self,
            index: child,
        }
    }

    pub fn iter_at<'a>(&'a self, index: usize) -> IterAt<'a, T> {
        IterAt {
            tree: self,
            stack: vec![index],
        }
    }

    pub fn iter(&self) -> slab::Iter<'_, TreeNode<T>> {
        self.slab.iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> slab::IterMut<'a, TreeNode<T>> {
        self.slab.iter_mut()
    }
}

impl<T> std::ops::Index<usize> for Tree<T> {
    type Output = TreeNode<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.slab[index]
    }
}

impl<T> std::ops::IndexMut<usize> for Tree<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.slab[index]
    }
}

pub struct Children<'a, T> {
    tree: &'a Tree<T>,
    index: Option<usize>,
}

impl<'a, T> Iterator for Children<'a, T> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index?;
        let node = &self.tree[index];
        self.index = node.next_sibling();

        Some(index)
    }
}

pub struct IterAt<'a, T> {
    tree: &'a Tree<T>,
    stack: Vec<usize>,
}

impl<'a, T> Iterator for IterAt<'a, T> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.stack.pop()?;

        self.stack.extend(self.tree.children(index));

        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn children<T: Copy>(tree: &Tree<T>, index: usize) -> Vec<T> {
        let i: Vec<_> = tree.children(index).collect();
        i.into_iter().map(|i| tree.get(&i).unwrap().value).collect()
    }

    fn iter<T: Copy>(tree: &Tree<T>, index: usize) -> Vec<T> {
        let i: Vec<_> = tree.iter(index).collect();
        i.into_iter().map(|i| tree.get(&i).unwrap().value).collect()
    }

    #[test]
    fn test_tree() {
        let mut tree = Tree::new(1);

        let two = tree.insert(&tree.root(), 2).unwrap();
        tree.insert(&two, 3).unwrap();
        let four = tree.insert(&two, 4).unwrap();

        tree.insert(&four, 5).unwrap();
        tree.insert(&four, 6).unwrap();
        tree.insert(&four, 7).unwrap();

        tree.insert(&tree.root(), 8).unwrap();

        assert_eq!(children(&tree, &tree.root()), vec![8, 2]);
        assert_eq!(children(&tree, &two), vec![4, 3]);
        assert_eq!(children(&tree, &four), vec![7, 6, 5]);
        assert_eq!(iter(&tree, &tree.root), vec![1, 2, 3, 4, 5, 6, 7, 8]);

        let new = tree.remove(&two).unwrap();
        assert_eq!(iter(&tree, &tree.root), vec![1, 8]);
        assert_eq!(iter(&new, &new.root), vec![2, 3, 4, 5, 6, 7]);
    }
}
