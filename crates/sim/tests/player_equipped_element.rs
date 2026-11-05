//! Hand-derived controls for the recovered equipped helper in Website ledger010.
use solomon_sim::Vec2;
use solomon_sim::player_presentation::{
    PlayerAttachmentPoses, PlayerSecondaryCastPresentation, player_equipped_element_plan,
};
use solomon_sim::spells::casting::PlayerWeaponKind;

#[test]
fn equipped_admission_uses_the_authored_socket_instead_of_body_heading() {
    let poses = PlayerAttachmentPoses { staff: 0, wand: 0, bare: None };
    // Staff keeps virtual XY. The ordinary Wand query reuses X for Y,
    // although its actual helper socket still keeps the authored full XY.
    for (kind, socket, front) in [
        (PlayerWeaponKind::Staff, Vec2::new(-32.5, -66.5), false),
        (PlayerWeaponKind::Wand, Vec2::new(14.5, -50.5), true),
    ] {
        let plan = player_equipped_element_plan(0, poses, kind, None, 0.0, 80.0);
        assert_eq!(plan.socket, socket);
        assert_eq!(plan.passes.front_base, front);
        assert_eq!(plan.passes.front_overlay, !front);
        assert!(!plan.overlay_after_head);
        assert!(plan.attachment_offset.x > 0.0);
    }
    // At bodyheading6 the Wand socket is below the -20 reference, so its
    // unshifted base survives and the low-phase overlay does not.
    let plan = player_equipped_element_plan(6, poses, PlayerWeaponKind::Wand, None, 0.0, 80.0);
    assert_eq!(plan.socket, Vec2::new(39.5, -5.0));
    assert!(plan.passes.front_base);
    assert!(!plan.passes.front_overlay);
}

#[test]
fn wand_and_staff_scale_one_common_element_and_damage_halo_without_early_rounding() {
    let poses = PlayerAttachmentPoses { staff: 0, wand: 2, bare: None };
    let staff = player_equipped_element_plan(6, poses, PlayerWeaponKind::Staff, None, 0.45, 0.0);
    let wand = player_equipped_element_plan(6, poses, PlayerWeaponKind::Wand, None, 0.45, 0.0);
    assert_eq!(staff.scale, 5.5);
    assert_eq!(wand.scale, 3.3000001907348633);
    assert!(wand.passes.front_overlay && wand.overlay_after_head);
    assert_ne!(
        wand.socket,
        player_equipped_element_plan(
            6,
            PlayerAttachmentPoses { wand: 0, ..poses },
            PlayerWeaponKind::Wand,
            None,
            0.45,
            0.0
        )
        .socket
    );
}

#[test]
fn cast_spin_uses_one_late_copy_and_zero_lateral_attachment_bob() {
    let poses = PlayerAttachmentPoses { staff: 9, wand: 0, bare: None };
    for kind in [PlayerWeaponKind::Staff, PlayerWeaponKind::Wand] {
        let plan = player_equipped_element_plan(
            6,
            poses,
            kind,
            Some(PlayerSecondaryCastPresentation::Spin),
            0.0,
            80.0,
        );
        assert!(!plan.passes.front_base);
        assert!(plan.passes.front_overlay && plan.overlay_after_head);
        assert_eq!(plan.attachment_offset, Vec2::new(0.0, 1.0));
    }
}
