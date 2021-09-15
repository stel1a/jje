// Copyright 2021 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use jujutsu_lib::backend::CommitId;
use jujutsu_lib::commit::Commit;
use jujutsu_lib::commit_builder::CommitBuilder;
use jujutsu_lib::repo_path::RepoPath;
use jujutsu_lib::rewrite::{DescendantRebaser, RebasedDescendant};
use jujutsu_lib::testutils;
use jujutsu_lib::testutils::CommitGraphBuilder;
use maplit::hashmap;
use test_case::test_case;

fn assert_in_place(rebased: Option<RebasedDescendant>, expected_old_commit: &Commit) {
    if let Some(RebasedDescendant::AlreadyInPlace(old_commit)) = rebased {
        assert_eq!(old_commit, *expected_old_commit);
    } else {
        panic!("expected in-place commit: {:?}", rebased);
    }
}

fn assert_ancestor(rebased: Option<RebasedDescendant>, expected_old_commit: &Commit) {
    if let Some(RebasedDescendant::AncestorOfDestination(old_commit)) = rebased {
        assert_eq!(old_commit, *expected_old_commit);
    } else {
        panic!("expected ancestor commit: {:?}", rebased);
    }
}

fn assert_rebased(
    rebased: Option<RebasedDescendant>,
    expected_old_commit: &Commit,
    expected_new_parents: &[CommitId],
) -> Commit {
    if let Some(RebasedDescendant::Rebased {
        old_commit,
        new_commit,
    }) = rebased
    {
        assert_eq!(old_commit, *expected_old_commit);
        assert_eq!(new_commit.change_id(), expected_old_commit.change_id());
        assert_eq!(&new_commit.parent_ids(), expected_new_parents);
        new_commit
    } else {
        panic!("expected rebased commit: {:?}", rebased);
    }
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_sideways(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 was replaced by commit 6. Commits 3-5 should be rebased.
    //
    // 6
    // | 4
    // | 3 5
    // | |/
    // | 2
    // |/
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit3]);
    let commit5 = graph_builder.commit_with_parents(&[&commit2]);
    let commit6 = graph_builder.commit_with_parents(&[&commit1]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() => vec![commit6.id().clone()]
        },
    );
    let new_commit3 = assert_rebased(rebaser.rebase_next(), &commit3, &[commit6.id().clone()]);
    assert_rebased(rebaser.rebase_next(), &commit4, &[new_commit3.id().clone()]);
    assert_rebased(rebaser.rebase_next(), &commit5, &[commit6.id().clone()]);
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 3);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_forward(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 was replaced by commit 6. Commits 3 and 5 should be rebased onto 6.
    // Commit 4 does not get rebased because it's an ancestor of the
    // destination. Commit 7 does not get replaced because it's already in
    // place.
    //
    // 7
    // 6 5
    // |/
    // 4 3
    // |/
    // 2
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit2]);
    let commit5 = graph_builder.commit_with_parents(&[&commit4]);
    let commit6 = graph_builder.commit_with_parents(&[&commit4]);
    let commit7 = graph_builder.commit_with_parents(&[&commit6]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() =>
            vec![commit6.id().clone()]
        },
    );
    assert_rebased(rebaser.rebase_next(), &commit3, &[commit6.id().clone()]);
    assert_ancestor(rebaser.rebase_next(), &commit4);
    assert_rebased(rebaser.rebase_next(), &commit5, &[commit6.id().clone()]);
    assert_ancestor(rebaser.rebase_next(), &commit6);
    assert_in_place(rebaser.rebase_next(), &commit7);
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 2);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_backward(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 3 was replaced by commit 2. Commit 4 should be rebased.
    //
    // 4
    // 3
    // 2
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit3]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit3.id().clone() => vec![commit2.id().clone()]
        },
    );
    assert_rebased(rebaser.rebase_next(), &commit4, &[commit2.id().clone()]);
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 1);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_internal_merge(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 was replaced by commit 6. Commits 3-5 should be rebased.
    //
    // 6
    // | 5
    // | |\
    // | 3 4
    // | |/
    // | 2
    // |/
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit2]);
    let commit5 = graph_builder.commit_with_parents(&[&commit3, &commit4]);
    let commit6 = graph_builder.commit_with_parents(&[&commit1]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() => vec![commit6.id().clone()]
        },
    );
    let new_commit3 = assert_rebased(rebaser.rebase_next(), &commit3, &[commit6.id().clone()]);
    let new_commit4 = assert_rebased(rebaser.rebase_next(), &commit4, &[commit6.id().clone()]);
    assert_rebased(
        rebaser.rebase_next(),
        &commit5,
        &[new_commit3.id().clone(), new_commit4.id().clone()],
    );
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 3);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_external_merge(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 3 was replaced by commit 6. Commits 5 should be rebased. The rebased
    // commit 5 should have 6 as first parent and commit 4 as second parent.
    //
    // 6
    // | 5
    // | |\
    // | 3 4
    // | |/
    // | 2
    // |/
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit2]);
    let commit5 = graph_builder.commit_with_parents(&[&commit3, &commit4]);
    let commit6 = graph_builder.commit_with_parents(&[&commit1]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit3.id().clone() => vec![commit6.id().clone()]
        },
    );
    assert_rebased(
        rebaser.rebase_next(),
        &commit5,
        &[commit6.id().clone(), commit4.id().clone()],
    );
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 1);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_degenerate_merge(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 was replaced by commit 1 (maybe it was pruned). Commit 4 should get
    // rebased to have only 3 as parent (not 1 and 3).
    //
    // 4
    // |\
    // 2 3
    // |/
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit1]);
    let commit4 = graph_builder.commit_with_parents(&[&commit2, &commit3]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() => vec![commit1.id().clone()]
        },
    );
    assert_rebased(rebaser.rebase_next(), &commit4, &[commit3.id().clone()]);
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 1);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_widen_merge(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 5 was replaced by commits 2 and 3 (maybe 5 was pruned). Commit 6
    // should get rebased to have 2, 3, and 4 as parents (in that order).
    //
    // 6
    // |\
    // 5 \
    // |\ \
    // 2 3 4
    //  \|/
    //   1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit1]);
    let commit4 = graph_builder.commit_with_parents(&[&commit1]);
    let commit5 = graph_builder.commit_with_parents(&[&commit2, &commit3]);
    let commit6 = graph_builder.commit_with_parents(&[&commit5, &commit4]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit5.id().clone() => vec![commit2.id().clone(), commit3.id().clone()]
        },
    );
    assert_rebased(
        rebaser.rebase_next(),
        &commit6,
        &[
            commit2.id().clone(),
            commit3.id().clone(),
            commit4.id().clone(),
        ],
    );
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 1);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_multiple_sideways(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 and commit 4 were both replaced by commit 6. Commit 3 and commit 5
    // should get rebased onto it.
    //
    // 3 5
    // 2 4 6
    // | |/
    // |/
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit1]);
    let commit5 = graph_builder.commit_with_parents(&[&commit4]);
    let commit6 = graph_builder.commit_with_parents(&[&commit1]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() => vec![commit6.id().clone()],
            commit4.id().clone() => vec![commit6.id().clone()],
        },
    );
    assert_rebased(rebaser.rebase_next(), &commit3, &[commit6.id().clone()]);
    assert_rebased(rebaser.rebase_next(), &commit5, &[commit6.id().clone()]);
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 2);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_multiple_swap(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 was replaced by commit 4 and commit 4 was replaced by commit 2.
    // Commit 3 and commit 5 should swap places.
    //
    // 3 5
    // 2 4
    // |/
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit1]);
    let commit5 = graph_builder.commit_with_parents(&[&commit4]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() => vec![commit4.id().clone()],
            commit4.id().clone() => vec![commit2.id().clone()],
        },
    );
    assert_rebased(rebaser.rebase_next(), &commit3, &[commit4.id().clone()]);
    assert_rebased(rebaser.rebase_next(), &commit5, &[commit2.id().clone()]);
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 2);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_multiple_forward_and_backward(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 was replaced by commit 4 and commit 6 was replaced by commit 3.
    // Commit 7 should be rebased onto commit 3. Commit 8 should be rebased onto
    // commit 4. Commits 3-4 should be left alone since they're ancestors of 4.
    // Commit 5 should be left alone since its already in place (as a descendant of
    // 4).
    //
    // 7
    // 6
    // 5
    // 4
    // 3 8
    // |/
    // 2
    // 1
    let mut tx = repo.start_transaction("test");
    let mut graph_builder = CommitGraphBuilder::new(&settings, tx.mut_repo());
    let commit1 = graph_builder.initial_commit();
    let commit2 = graph_builder.commit_with_parents(&[&commit1]);
    let commit3 = graph_builder.commit_with_parents(&[&commit2]);
    let commit4 = graph_builder.commit_with_parents(&[&commit3]);
    let commit5 = graph_builder.commit_with_parents(&[&commit4]);
    let commit6 = graph_builder.commit_with_parents(&[&commit5]);
    let commit7 = graph_builder.commit_with_parents(&[&commit6]);
    let commit8 = graph_builder.commit_with_parents(&[&commit2]);

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() => vec![commit4.id().clone()],
            commit6.id().clone() => vec![commit3.id().clone()],
        },
    );
    assert_ancestor(rebaser.rebase_next(), &commit3);
    assert_ancestor(rebaser.rebase_next(), &commit4);
    assert_in_place(rebaser.rebase_next(), &commit5);
    assert_rebased(rebaser.rebase_next(), &commit7, &[commit3.id().clone()]);
    assert_rebased(rebaser.rebase_next(), &commit8, &[commit4.id().clone()]);
    assert!(rebaser.rebase_next().is_none());
    assert_eq!(rebaser.rebased().len(), 2);

    tx.discard();
}

#[test_case(false ; "local backend")]
#[test_case(true ; "git backend")]
fn test_rebase_descendants_contents(use_git: bool) {
    let settings = testutils::user_settings();
    let (_temp_dir, repo) = testutils::init_repo(&settings, use_git);

    // Commit 2 was replaced by commit 4. Commit 3 should have the changes from
    // commit 3 and commit 4, but not the changes from commit 2.
    //
    // 4
    // | 3
    // | 2
    // |/
    // 1
    let mut tx = repo.start_transaction("test");
    let path1 = RepoPath::from_internal_string("file1");
    let tree1 = testutils::create_tree(&repo, &[(&path1, "content")]);
    let commit1 = CommitBuilder::for_new_commit(&settings, repo.store(), tree1.id().clone())
        .write_to_repo(tx.mut_repo());
    let path2 = RepoPath::from_internal_string("file2");
    let tree2 = testutils::create_tree(&repo, &[(&path2, "content")]);
    let commit2 = CommitBuilder::for_new_commit(&settings, repo.store(), tree2.id().clone())
        .set_parents(vec![commit1.id().clone()])
        .write_to_repo(tx.mut_repo());
    let path3 = RepoPath::from_internal_string("file3");
    let tree3 = testutils::create_tree(&repo, &[(&path3, "content")]);
    let commit3 = CommitBuilder::for_new_commit(&settings, repo.store(), tree3.id().clone())
        .set_parents(vec![commit2.id().clone()])
        .write_to_repo(tx.mut_repo());
    let path4 = RepoPath::from_internal_string("file4");
    let tree4 = testutils::create_tree(&repo, &[(&path4, "content")]);
    let commit4 = CommitBuilder::for_new_commit(&settings, repo.store(), tree4.id().clone())
        .set_parents(vec![commit1.id().clone()])
        .write_to_repo(tx.mut_repo());

    let mut rebaser = DescendantRebaser::new(
        &settings,
        tx.mut_repo(),
        hashmap! {
            commit2.id().clone() => vec![commit4.id().clone()]
        },
    );
    rebaser.rebase_all();
    let rebased = rebaser.rebased();
    assert_eq!(rebased.len(), 1);
    let new_commit3 = repo
        .store()
        .get_commit(rebased.get(commit3.id()).unwrap())
        .unwrap();

    assert_eq!(
        new_commit3.tree().path_value(&path3),
        commit3.tree().path_value(&path3)
    );
    assert_eq!(
        new_commit3.tree().path_value(&path4),
        commit4.tree().path_value(&path4)
    );
    assert_ne!(
        new_commit3.tree().path_value(&path2),
        commit2.tree().path_value(&path2)
    );

    tx.discard();
}
