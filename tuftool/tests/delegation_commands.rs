// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

mod test_utils;

use assert_cmd::Command;
use chrono::{Duration, Utc};
use std::fs::File;
use std::path::Path;
use tempfile::TempDir;
use tough::{ExpirationEnforcement, Limits, Repository, Settings};

fn create_repo<P: AsRef<Path>>(repo_dir: P) {
    let timestamp_expiration = Utc::now().checked_add_signed(Duration::days(1)).unwrap();
    let timestamp_version: u64 = 31;
    let snapshot_expiration = Utc::now().checked_add_signed(Duration::days(2)).unwrap();
    let snapshot_version: u64 = 25;
    let targets_expiration = Utc::now().checked_add_signed(Duration::days(3)).unwrap();
    let targets_version: u64 = 17;
    let targets_input_dir = test_utils::test_data()
        .join("tuf-reference-impl")
        .join("targets");
    let root_json = test_utils::test_data().join("simple-rsa").join("root.json");
    let root_key = test_utils::test_data().join("snakeoil.pem");

    // Create a repo using tuftool and the reference tuf implementation data
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "create",
            "-t",
            targets_input_dir.to_str().unwrap(),
            "-o",
            repo_dir.as_ref().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--targets-expires",
            targets_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", targets_version).as_str(),
            "--snapshot-expires",
            snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", snapshot_version).as_str(),
            "--timestamp-expires",
            timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", timestamp_version).as_str(),
        ])
        .assert()
        .success();
}

#[test]
// Ensure we can create a role, add the role to parent metadata, and sign repo
// Structure targets -> A -> B
fn create_add_role_command() {
    let root_json = test_utils::test_data().join("simple-rsa").join("root.json");
    let root_key = test_utils::test_data().join("snakeoil.pem");
    let targets_key = test_utils::test_data().join("targetskey");
    let targets_key1 = test_utils::test_data().join("targetskey-1");
    let repo_dir = TempDir::new().unwrap();

    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let new_targets_expiration = Utc::now().checked_add_signed(Duration::days(6)).unwrap();
    let new_targets_version: u64 = 170;

    // Create a repo using tuftool and the reference tuf implementation data
    create_repo(repo_dir.path());

    // Set new expiration date for the new role
    let expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let metadata_base_url = &test_utils::dir_url(repo_dir.path().join("metadata"));
    let meta_out = TempDir::new().unwrap();

    // create role A
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "create-role",
            "-o",
            meta_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let new_repo_dir = TempDir::new().unwrap();
    // add role to targets metadata and sign entire repo
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "add-role",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&meta_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "A",
            "-t",
            "1",
            "-v",
            "2",
            "--sign-all",
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(new_repo_dir.path().join("targets"));
    let repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(&root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();
    // Make sure `A` is added as a role
    assert!(repo.delegated_role("A").is_some());

    let create_out = TempDir::new().unwrap();
    // create role B
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "B",
            "create-role",
            "-o",
            create_out.path().to_str().unwrap(),
            "-k",
            targets_key1.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let add_b_out = TempDir::new().unwrap();
    // add role B to A metadata and sign A meta
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "add-role",
            "-o",
            add_b_out.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&create_out.path().join("metadata")).as_str(),
            "-k",
            targets_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "B",
            "-t",
            "1",
            "-v",
            "2",
        ])
        .assert()
        .success();

    // update repo with new metadata

    let update_out = TempDir::new().unwrap();

    // Update the repo we just created
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "-o",
            update_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            new_targets_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", new_targets_version).as_str(),
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
            "--role",
            "A",
            "-i",
            test_utils::dir_url(&add_b_out.path().join("metadata")).as_str(),
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(update_out.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(update_out.path().join("targets"));
    let repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();

    // Make sure `B` is added as a role
    assert!(repo.delegated_role("B").is_some());
}
#[test]
// Ensure we can update targets of delegated roles
fn update_target_command() {
    let root_json = test_utils::test_data().join("simple-rsa").join("root.json");
    let root_key = test_utils::test_data().join("snakeoil.pem");
    let targets_key = test_utils::test_data().join("targetskey");
    let repo_dir = TempDir::new().unwrap();

    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;

    // Create a repo using tuftool and the reference tuf implementation data
    create_repo(repo_dir.path());

    // Set new expiration date for the new role
    let expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let metadata_base_url = &test_utils::dir_url(repo_dir.path().join("metadata"));
    let meta_out = TempDir::new().unwrap();

    // create role A
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "create-role",
            "-o",
            meta_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let new_repo_dir = TempDir::new().unwrap();
    // add role to targets metadata and sign entire repo
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "add-role",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&meta_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "A",
            "-t",
            "1",
            "-v",
            "2",
            "--sign-all",
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
        ])
        .assert()
        .success();

    // Update A's targets
    let ut_out = TempDir::new().unwrap();
    let meta_out_url = test_utils::dir_url(&ut_out.path().join("metadata"));
    let targets_out_url = ut_out.path().join("targets");
    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));
    let targets_input_dir = test_utils::test_data().join("targets");
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "update-delegated-targets",
            "-o",
            ut_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "-t",
            targets_input_dir.to_str().unwrap(),
            "-e",
            "in 5 days",
            "-v",
            "2",
        ])
        .assert()
        .success();

    // update repo with new metadata
    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let new_targets_expiration = Utc::now().checked_add_signed(Duration::days(6)).unwrap();
    let new_targets_version: u64 = 170;
    let update_out = TempDir::new().unwrap();

    // Update the repo we just created
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "-o",
            update_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            new_targets_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", new_targets_version).as_str(),
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
            "--role",
            "A",
            "-i",
            &meta_out_url,
            "-t",
            targets_out_url.to_str().unwrap(),
            "-f",
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(update_out.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(update_out.path().join("targets"));
    let repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();

    // Make sure we can read new target
    assert_eq!(
        test_utils::read_to_end(repo.read_target("file4.txt").unwrap().unwrap()),
        &b"This is an example target file."[..]
    );
}

#[test]
// Ensure we can add keys to A and B
// Adds new key to A and signs with it
fn add_key_command() {
    let root_json = test_utils::test_data().join("simple-rsa").join("root.json");
    let root_key = test_utils::test_data().join("snakeoil.pem");
    let targets_key = test_utils::test_data().join("targetskey");
    let targets_key1 = test_utils::test_data().join("targetskey-1");
    let repo_dir = TempDir::new().unwrap();

    // Create a repo using tuftool and the reference tuf implementation data
    create_repo(repo_dir.path());

    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let metadata_base_url = &test_utils::dir_url(repo_dir.path().join("metadata"));
    let meta_out = TempDir::new().unwrap();

    // create role A
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "create-role",
            "-o",
            meta_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let new_repo_dir = TempDir::new().unwrap();
    // add role to targets metadata and sign entire repo
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "add-role",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&meta_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "A",
            "-t",
            "1",
            "-v",
            "2",
            "--sign-all",
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
        ])
        .assert()
        .success();
    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));

    // add key to A
    let key_out = TempDir::new().unwrap();
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "add-key",
            "-o",
            key_out.path().to_str().unwrap(),
            "--new-key",
            targets_key1.to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
            "--delegated-role",
            "A",
        ])
        .assert()
        .success();

    //sign A's key addition as repo owner
    let new_repo_dir = TempDir::new().unwrap();
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "--role",
            "targets",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&key_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            expiration.to_rfc3339().as_str(),
            "--targets-version",
            "1",
            "--snapshot-expires",
            expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            "1",
            "--timestamp-expires",
            expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            "1",
        ])
        .assert()
        .success();

    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));

    let create_out = TempDir::new().unwrap();
    // create role B
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "B",
            "create-role",
            "-o",
            create_out.path().to_str().unwrap(),
            "-k",
            targets_key1.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let add_b_out = TempDir::new().unwrap();
    // add role B to A metadata and sign A meta with the added key
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "add-role",
            "-o",
            add_b_out.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&create_out.path().join("metadata")).as_str(),
            "-k",
            targets_key1.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "B",
            "-t",
            "1",
            "-v",
            "2",
        ])
        .assert()
        .success();

    // update repo with new metadata

    let update_out = TempDir::new().unwrap();

    // Update the repo we just created
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "-o",
            update_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", new_snapshot_version).as_str(),
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
            "--role",
            "A",
            "-i",
            test_utils::dir_url(&add_b_out.path().join("metadata")).as_str(),
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(update_out.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(update_out.path().join("targets"));
    let _repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();
}

#[test]
// Ensure we can remove keys from A
// Adds removes a key from A and makes sure we can't sign with it
fn remove_key_command() {
    let root_json = test_utils::test_data().join("simple-rsa").join("root.json");
    let root_key = test_utils::test_data().join("snakeoil.pem");
    let targets_key = test_utils::test_data().join("targetskey");
    let targets_key1 = test_utils::test_data().join("targetskey-1");
    let repo_dir = TempDir::new().unwrap();

    // Create a repo using tuftool and the reference tuf implementation data
    create_repo(repo_dir.path());

    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let metadata_base_url = &test_utils::dir_url(repo_dir.path().join("metadata"));
    let meta_out = TempDir::new().unwrap();

    // create role A
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "create-role",
            "-o",
            meta_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "-k",
            targets_key1.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let new_repo_dir = TempDir::new().unwrap();
    // add role to targets metadata and sign entire repo
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "add-role",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&meta_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "A",
            "-t",
            "1",
            "-v",
            "2",
            "--sign-all",
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
        ])
        .assert()
        .success();

    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));

    // remove key from A
    let key_out = TempDir::new().unwrap();
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "remove-key",
            "-o",
            key_out.path().to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
            "--keyid",
            "9d25bd7d096386713d823447e9920ea4b807bd95d1bf7a0d05a00979ab5eec00",
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--delegated-role",
            "A",
        ])
        .assert()
        .success();

    //sign A's key removal as repo owner
    let new_repo_dir = TempDir::new().unwrap();
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "--role",
            "targets",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&key_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            expiration.to_rfc3339().as_str(),
            "--targets-version",
            "1",
            "--snapshot-expires",
            expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            "1",
            "--timestamp-expires",
            expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            "1",
        ])
        .assert()
        .success();

    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));

    let create_out = TempDir::new().unwrap();
    // create role B
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "B",
            "create-role",
            "-o",
            create_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let add_b_out = TempDir::new().unwrap();
    // add role B to A metadata and sign A meta with the removed key
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "add-role",
            "-o",
            add_b_out.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&create_out.path().join("metadata")).as_str(),
            "-k",
            targets_key1.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "B",
            "-t",
            "1",
            "-v",
            "2",
        ])
        .assert()
        .failure();
}

#[test]
// Ensure we can remove a role
fn remove_role_command() {
    let root_json = test_utils::test_data().join("simple-rsa").join("root.json");
    let root_key = test_utils::test_data().join("snakeoil.pem");
    let targets_key = test_utils::test_data().join("targetskey");
    let targets_key1 = test_utils::test_data().join("targetskey-1");
    let repo_dir = TempDir::new().unwrap();

    // Create a repo using tuftool and the reference tuf implementation data
    create_repo(repo_dir.path());

    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let metadata_base_url = &test_utils::dir_url(repo_dir.path().join("metadata"));
    let meta_out = TempDir::new().unwrap();

    // create role A
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "create-role",
            "-o",
            meta_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let new_repo_dir = TempDir::new().unwrap();
    // add role to targets metadata and sign entire repo
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "add-role",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&meta_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "A",
            "-t",
            "1",
            "-v",
            "2",
            "--sign-all",
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(new_repo_dir.path().join("targets"));
    let repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(&root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();
    // Make sure `A` is added as a role
    assert!(repo.delegated_role("A").is_some());

    let create_out = TempDir::new().unwrap();
    // create role B
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "B",
            "create-role",
            "-o",
            create_out.path().to_str().unwrap(),
            "-k",
            targets_key1.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let add_b_out = TempDir::new().unwrap();
    // add role B to A metadata
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "add-role",
            "-o",
            add_b_out.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&create_out.path().join("metadata")).as_str(),
            "-k",
            targets_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "B",
            "-t",
            "1",
            "-v",
            "2",
        ])
        .assert()
        .success();

    // update repo with new metadata
    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let new_targets_expiration = Utc::now().checked_add_signed(Duration::days(6)).unwrap();
    let new_targets_version: u64 = 170;
    let update_out = TempDir::new().unwrap();

    // Update the repo we just created
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "-o",
            update_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            new_targets_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", new_targets_version).as_str(),
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
            "--role",
            "A",
            "-i",
            test_utils::dir_url(&add_b_out.path().join("metadata")).as_str(),
        ])
        .assert()
        .success();

    // Remove B from the repo
    let remove_b_out = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(update_out.path().join("metadata"));

    // remove role B from A metadata and sign A meta
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "remove",
            "-o",
            remove_b_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--delegated-role",
            "B",
            "-e",
            "in 4 days",
            "-v",
            "325",
        ])
        .assert()
        .success();

    // update repo with new metadata
    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let new_targets_expiration = Utc::now().checked_add_signed(Duration::days(6)).unwrap();
    let new_targets_version: u64 = 170;
    let update_out = TempDir::new().unwrap();

    // Update the repo we just created
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "-o",
            update_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            new_targets_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", new_targets_version).as_str(),
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
            "--role",
            "A",
            "-i",
            test_utils::dir_url(&remove_b_out.path().join("metadata")).as_str(),
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(update_out.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(update_out.path().join("targets"));
    let repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();

    // Make sure `B` is removed
    assert!(repo.delegated_role("B").is_none());
}

#[test]
// Ensure we can remove a role
fn remove_role_recursive_command() {
    let root_json = test_utils::test_data().join("simple-rsa").join("root.json");
    let root_key = test_utils::test_data().join("snakeoil.pem");
    let targets_key = test_utils::test_data().join("targetskey");
    let targets_key1 = test_utils::test_data().join("targetskey-1");
    let repo_dir = TempDir::new().unwrap();

    // Create a repo using tuftool and the reference tuf implementation data
    create_repo(repo_dir.path());

    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let metadata_base_url = &test_utils::dir_url(repo_dir.path().join("metadata"));
    let meta_out = TempDir::new().unwrap();

    // create role A
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "create-role",
            "-o",
            meta_out.path().to_str().unwrap(),
            "-k",
            targets_key.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let new_repo_dir = TempDir::new().unwrap();
    // add role to targets metadata and sign entire repo
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "add-role",
            "-o",
            new_repo_dir.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&meta_out.path().join("metadata")).as_str(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "A",
            "-t",
            "1",
            "-v",
            "2",
            "--sign-all",
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(new_repo_dir.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(new_repo_dir.path().join("targets"));
    let repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(&root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();
    // Make sure `A` is added as a role
    assert!(repo.delegated_role("A").is_some());

    let create_out = TempDir::new().unwrap();
    // create role B
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "B",
            "create-role",
            "-o",
            create_out.path().to_str().unwrap(),
            "-k",
            targets_key1.to_str().unwrap(),
            "-e",
            expiration.to_rfc3339().as_str(),
            "-v",
            "1",
        ])
        .assert()
        .success();

    let add_b_out = TempDir::new().unwrap();
    // add role B to A metadata
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "A",
            "add-role",
            "-o",
            add_b_out.path().to_str().unwrap(),
            "-i",
            test_utils::dir_url(&create_out.path().join("metadata")).as_str(),
            "-k",
            targets_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "-e",
            expiration.to_rfc3339().as_str(),
            "--delegated-role",
            "B",
            "-t",
            "1",
            "-v",
            "2",
        ])
        .assert()
        .success();

    // update repo with new metadata
    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let new_targets_expiration = Utc::now().checked_add_signed(Duration::days(6)).unwrap();
    let new_targets_version: u64 = 170;
    let update_out = TempDir::new().unwrap();

    // Update the repo we just created
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "-o",
            update_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            new_targets_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", new_targets_version).as_str(),
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
            "--role",
            "A",
            "-i",
            test_utils::dir_url(&add_b_out.path().join("metadata")).as_str(),
        ])
        .assert()
        .success();

    // Remove B from the repo
    let remove_b_out = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(update_out.path().join("metadata"));

    // remove role B from A metadata and sign A meta
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "delegation",
            "--signing-role",
            "targets",
            "remove",
            "-o",
            remove_b_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--delegated-role",
            "B",
            "-e",
            "in 4 days",
            "-v",
            "325",
            "--recursive",
        ])
        .assert()
        .success();

    // update repo with new metadata
    // Set new expiration dates and version numbers for the update command
    let new_timestamp_expiration = Utc::now().checked_add_signed(Duration::days(4)).unwrap();
    let new_timestamp_version: u64 = 310;
    let new_snapshot_expiration = Utc::now().checked_add_signed(Duration::days(5)).unwrap();
    let new_snapshot_version: u64 = 250;
    let new_targets_expiration = Utc::now().checked_add_signed(Duration::days(6)).unwrap();
    let new_targets_version: u64 = 170;
    let update_out = TempDir::new().unwrap();

    // Update the repo we just created
    Command::cargo_bin("tuftool")
        .unwrap()
        .args(&[
            "update",
            "-o",
            update_out.path().to_str().unwrap(),
            "-k",
            root_key.to_str().unwrap(),
            "--root",
            root_json.to_str().unwrap(),
            "--metadata-url",
            updated_metadata_base_url,
            "--targets-expires",
            new_targets_expiration.to_rfc3339().as_str(),
            "--targets-version",
            format!("{}", new_targets_version).as_str(),
            "--snapshot-expires",
            new_snapshot_expiration.to_rfc3339().as_str(),
            "--snapshot-version",
            format!("{}", new_snapshot_version).as_str(),
            "--timestamp-expires",
            new_timestamp_expiration.to_rfc3339().as_str(),
            "--timestamp-version",
            format!("{}", new_timestamp_version).as_str(),
            "--role",
            "targets",
            "-i",
            test_utils::dir_url(&remove_b_out.path().join("metadata")).as_str(),
        ])
        .assert()
        .success();

    // Load the updated repo
    let temp_datastore = TempDir::new().unwrap();
    let updated_metadata_base_url = &test_utils::dir_url(update_out.path().join("metadata"));
    let updated_targets_base_url = &test_utils::dir_url(update_out.path().join("targets"));
    let repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: File::open(root_json).unwrap(),
            datastore: temp_datastore.as_ref(),
            metadata_base_url: updated_metadata_base_url,
            targets_base_url: updated_targets_base_url,
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();

    // Make sure `A` and `B` are removed
    assert!(repo.delegated_role("A").is_none());
    assert!(repo.delegated_role("B").is_none());
}
