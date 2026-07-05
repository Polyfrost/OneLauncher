use oneclient_core::patch::Patch;

#[test]
fn patch_apply_to_option() {
	let mut slot = Some(42u32);

	Patch::Unchanged.apply_to_option(&mut slot);
	assert_eq!(slot, Some(42));

	Patch::Set(99).apply_to_option(&mut slot);
	assert_eq!(slot, Some(99));

	Patch::<u32>::Clear.apply_to_option(&mut slot);
	assert_eq!(slot, None);
}

#[test]
fn patch_command_empty_string_clears() {
	let mut slot: Option<String> = Some("wrapper".into());

	Patch::Set("   ".to_owned()).apply_to_command_option(&mut slot);
	assert_eq!(slot, Some("".to_owned()));
}

#[test]
fn patch_into_db_patch() {
	assert_eq!(Patch::<String>::Unchanged.into_db_patch(), None);
	assert_eq!(Patch::<String>::Clear.into_db_patch(), Some(None));
	assert_eq!(
		Patch::Set("x".to_string()).into_db_patch(),
		Some(Some("x".to_string()))
	);
}
