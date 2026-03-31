use proc_macro2::Span;
use syn::{
    visit_mut::{self, VisitMut},
    File, Ident, ItemUse, UseGlob, UseGroup, UseName, UsePath, UseRename, UseTree,
};

use crate::rename::Rename;

#[derive(Debug, Clone)]
pub struct Warning {
    pub message: String,
}

// ---------------------------------------------------------------------------
// UseEntry: a fully-expanded use-leaf (path + optional alias or glob marker)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Leaf {
    Name,
    Alias(String),
    Glob,
}

#[derive(Debug, Clone)]
struct UseLeaf {
    segs: Vec<String>,
    leaf: Leaf,
}

fn collect_leaves(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<UseLeaf>) {
    match tree {
        UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            collect_leaves(&p.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(n) => {
            let mut segs = prefix.clone();
            segs.push(n.ident.to_string());
            out.push(UseLeaf { segs, leaf: Leaf::Name });
        }
        UseTree::Rename(r) => {
            let mut segs = prefix.clone();
            segs.push(r.ident.to_string());
            out.push(UseLeaf {
                segs,
                leaf: Leaf::Alias(r.rename.to_string()),
            });
        }
        UseTree::Group(g) => {
            for item in &g.items {
                collect_leaves(item, prefix, out);
            }
        }
        UseTree::Glob(_) => {
            out.push(UseLeaf {
                segs: prefix.clone(),
                leaf: Leaf::Glob,
            });
        }
    }
}

fn build_tree(leaves: Vec<UseLeaf>) -> UseTree {
    build_tree_inner(&leaves, 0)
}

fn build_tree_inner(leaves: &[UseLeaf], depth: usize) -> UseTree {
    assert!(!leaves.is_empty());

    if leaves.len() == 1 {
        return build_leaf_at_depth(&leaves[0], depth);
    }

    // Find common prefix length beyond `depth`
    let first = &leaves[0].segs;
    let common = (depth..first.len().saturating_sub(1))
        .take_while(|&i| leaves.iter().all(|l| l.segs.get(i) == first.get(i)))
        .count();
    let common_end = depth + common;

    if common_end > depth {
        // Emit common path segments, then recurse
        let subtree = if common_end + 1 == first.len() {
            // Next is the leaf level — group
            build_group_at(leaves, common_end)
        } else {
            build_tree_inner(leaves, common_end)
        };
        wrap_path(&first[depth..common_end], subtree)
    } else {
        build_group_at(leaves, depth)
    }
}

fn wrap_path(segs: &[String], inner: UseTree) -> UseTree {
    let mut tree = inner;
    for seg in segs.iter().rev() {
        tree = UseTree::Path(UsePath {
            ident: Ident::new(seg, Span::call_site()),
            colon2_token: Default::default(),
            tree: Box::new(tree),
        });
    }
    tree
}

fn build_group_at(leaves: &[UseLeaf], depth: usize) -> UseTree {
    // Group leaves by their segment at `depth`
    // Partition into groups sharing segment[depth]
    let mut groups: Vec<(String, Vec<&UseLeaf>)> = Vec::new();
    for leaf in leaves {
        let seg = leaf.segs.get(depth).cloned().unwrap_or_default();
        if let Some(g) = groups.iter_mut().find(|(s, _)| s == &seg) {
            g.1.push(leaf);
        } else {
            groups.push((seg, vec![leaf]));
        }
    }

    let items: Vec<UseTree> = groups
        .into_iter()
        .map(|(_, group_leaves)| {
            let owned: Vec<UseLeaf> = group_leaves.into_iter().cloned().collect();
            if owned.len() == 1 {
                build_leaf_at_depth(&owned[0], depth)
            } else {
                build_tree_inner(&owned, depth)
            }
        })
        .collect();

    let mut punct = syn::punctuated::Punctuated::new();
    for item in items {
        punct.push(item);
    }
    UseTree::Group(UseGroup {
        brace_token: Default::default(),
        items: punct,
    })
}

fn build_leaf_at_depth(leaf: &UseLeaf, depth: usize) -> UseTree {
    let remaining = &leaf.segs[depth..];
    match &leaf.leaf {
        Leaf::Glob => {
            // Wrap remaining path segments then glob
            let glob = UseTree::Glob(UseGlob {
                star_token: Default::default(),
            });
            wrap_path(remaining, glob)
        }
        Leaf::Name => {
            if remaining.is_empty() {
                // Shouldn't happen for well-formed trees
                UseTree::Name(UseName {
                    ident: Ident::new("_", Span::call_site()),
                })
            } else if remaining.len() == 1 {
                UseTree::Name(UseName {
                    ident: Ident::new(&remaining[0], Span::call_site()),
                })
            } else {
                let last = UseTree::Name(UseName {
                    ident: Ident::new(remaining.last().unwrap(), Span::call_site()),
                });
                wrap_path(&remaining[..remaining.len() - 1], last)
            }
        }
        Leaf::Alias(alias) => {
            if remaining.is_empty() {
                UseTree::Rename(UseRename {
                    ident: Ident::new("_", Span::call_site()),
                    as_token: Default::default(),
                    rename: Ident::new(alias, Span::call_site()),
                })
            } else if remaining.len() == 1 {
                UseTree::Rename(UseRename {
                    ident: Ident::new(&remaining[0], Span::call_site()),
                    as_token: Default::default(),
                    rename: Ident::new(alias, Span::call_site()),
                })
            } else {
                let last = UseTree::Rename(UseRename {
                    ident: Ident::new(remaining.last().unwrap(), Span::call_site()),
                    as_token: Default::default(),
                    rename: Ident::new(alias, Span::call_site()),
                });
                wrap_path(&remaining[..remaining.len() - 1], last)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main rewriter
// ---------------------------------------------------------------------------

pub struct Rewriter<'a> {
    pub rename: &'a Rename,
    pub short_name_bound: bool,
    pub glob_warning: bool,
    pub warnings: Vec<Warning>,
    pub changed: bool,
}

impl<'a> Rewriter<'a> {
    pub fn new(rename: &'a Rename) -> Self {
        Rewriter {
            rename,
            short_name_bound: false,
            glob_warning: false,
            warnings: Vec::new(),
            changed: false,
        }
    }

    pub fn rewrite_file(&mut self, file: &mut File) {
        // Pass 1: rewrite use declarations
        for item in &mut file.items {
            if let syn::Item::Use(item_use) = item {
                self.rewrite_use_item(item_use);
            }
        }
        // Pass 2: rewrite paths in expressions / types (VisitMut)
        // We skip item_use nodes in visit_item_use_mut
        visit_mut::visit_file_mut(self, file);
    }

    fn rewrite_use_item(&mut self, item: &mut ItemUse) {
        let mut prefix = Vec::new();
        let mut leaves = Vec::new();
        collect_leaves(&item.tree, &mut prefix, &mut leaves);

        let from = &self.rename.from.0;
        let to = &self.rename.to.0;

        let mut any_changed = false;
        let mut new_leaves = Vec::new();

        for leaf in leaves {
            match &leaf.leaf {
                Leaf::Glob => {
                    // Check if glob parent matches `from`'s parent
                    if let Some(parent) = self.rename.from.parent() {
                        if leaf.segs == parent.0 {
                            if !self.glob_warning {
                                self.glob_warning = true;
                                self.warnings.push(Warning {
                                    message: format!(
                                        "glob use (use {}::*) found — short name bindings cannot be tracked, manual check required",
                                        leaf.segs.join("::")
                                    ),
                                });
                            }
                        }
                    }
                    new_leaves.push(leaf);
                }
                Leaf::Name => {
                    if leaf.segs == from.as_slice() {
                        // Direct match — rewrite to `to`
                        any_changed = true;
                        if self.rename.name_changed() {
                            self.short_name_bound = true;
                        }
                        new_leaves.push(UseLeaf { segs: to.clone(), leaf: Leaf::Name });
                    } else if leaf.segs.starts_with(from.as_slice()) {
                        // Prefix match (e.g. use a::b::OldName::SubItem)
                        let tail = leaf.segs[from.len()..].to_vec();
                        let mut new_segs = to.clone();
                        new_segs.extend(tail);
                        any_changed = true;
                        new_leaves.push(UseLeaf { segs: new_segs, leaf: Leaf::Name });
                    } else {
                        new_leaves.push(leaf);
                    }
                }
                Leaf::Alias(alias) => {
                    if leaf.segs == from.as_slice() {
                        any_changed = true;
                        // alias kept, path changed — don't set short_name_bound
                        new_leaves.push(UseLeaf {
                            segs: to.clone(),
                            leaf: Leaf::Alias(alias.clone()),
                        });
                    } else if leaf.segs.starts_with(from.as_slice()) {
                        let tail = leaf.segs[from.len()..].to_vec();
                        let mut new_segs = to.clone();
                        new_segs.extend(tail);
                        any_changed = true;
                        new_leaves.push(UseLeaf {
                            segs: new_segs,
                            leaf: Leaf::Alias(alias.clone()),
                        });
                    } else {
                        new_leaves.push(leaf);
                    }
                }
            }
        }

        if any_changed {
            self.changed = true;
            item.tree = build_tree(new_leaves);
        }
    }
}

impl<'a> VisitMut for Rewriter<'a> {
    fn visit_item_use_mut(&mut self, _node: &mut ItemUse) {
        // Already handled in pass 1 — skip
    }

    fn visit_path_mut(&mut self, node: &mut syn::Path) {
        let segs: Vec<String> = node.segments.iter().map(|s| s.ident.to_string()).collect();
        let from = &self.rename.from.0;
        let to = &self.rename.to.0;

        let rewritten = if segs.len() >= from.len() && segs[..from.len()] == *from.as_slice() {
            // Absolute path — rewrite prefix
            let mut new_segs = to.clone();
            new_segs.extend_from_slice(&segs[from.len()..]);
            Some(new_segs)
        } else if !segs.is_empty()
            && self.short_name_bound
            && self.rename.name_changed()
            && segs[0] == self.rename.old_name()
        {
            // Short name — rename first segment, keep rest (e.g. OldName::new() → NewName::new())
            let mut new_segs = vec![self.rename.new_name().to_string()];
            new_segs.extend_from_slice(&segs[1..]);
            Some(new_segs)
        } else {
            None
        };

        if let Some(new_segs) = rewritten {
            rebuild_path(node, &new_segs);
            self.changed = true;
        }

        // Don't recurse — we handled the whole path already
    }
}

fn rebuild_path(path: &mut syn::Path, new_segs: &[String]) {
    let old_args: Vec<syn::PathArguments> = path
        .segments
        .iter()
        .map(|s| s.arguments.clone())
        .collect();

    path.segments.clear();
    for (i, seg_name) in new_segs.iter().enumerate() {
        // Preserve generic arguments at matching positions
        let arguments = old_args.get(i).cloned().unwrap_or(syn::PathArguments::None);
        path.segments.push(syn::PathSegment {
            ident: Ident::new(seg_name, Span::call_site()),
            arguments,
        });
    }
}
