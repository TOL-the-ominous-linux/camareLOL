"""Unit tests for the Python automation tools."""

from __future__ import annotations

import unittest

from camarelol_tools import config_validate as cv
from camarelol_tools import hw_info, package_recommend


def base_profile() -> dict:
    return {
        "schema_version": 1,
        "firmware": "uefi",
        "filesystem": "ext4",
        "bootloader": "grub",
        "desktop": "plasma",
        "display_server": "wayland",
        "display_manager": "sddm",
        "system_locale": "en_US.UTF-8",
        "keymap": "us",
        "timezone": "UTC",
        "hostname": "archbox",
        "target_disk": {"path": "/dev/vda", "identity": "vm-disk-1", "size_bytes": 20 << 30},
        "root_login_allowed": True,
        "users": ["alice"],
        "admin_users": ["alice"],
        "packages": {"aur_enabled": False},
        "services": {"audio": "pipewire"},
    }


class TestConfigValidation(unittest.TestCase):
    def test_valid_profile(self):
        result = cv.validate_profile(base_profile())
        self.assertTrue(result, [str(p) for p in result.problems])

    def test_unknown_schema_version_is_fatal(self):
        profile = base_profile()
        profile["schema_version"] = 99
        result = cv.validate_profile(profile)
        self.assertFalse(result)
        self.assertEqual(result.problems[0].code, "unsupported_schema")

    def test_wayland_only_desktop_on_x11(self):
        profile = base_profile()
        profile["desktop"] = "sway"
        profile["display_server"] = "x11"
        result = cv.validate_profile(profile)
        self.assertFalse(result)

    def test_locked_out_admin_is_refused(self):
        profile = base_profile()
        profile["root_login_allowed"] = False
        profile["admin_users"] = []
        result = cv.validate_profile(profile)
        self.assertFalse(result)

    def test_admin_must_be_a_user(self):
        profile = base_profile()
        profile["admin_users"] = ["ghost"]
        result = cv.validate_profile(profile)
        self.assertFalse(result)

    def test_secrets_are_rejected(self):
        profile = base_profile()
        profile["root_password"] = "hunter2"
        result = cv.validate_profile(profile)
        self.assertFalse(result)
        self.assertTrue(any("secret" in str(p) or "password" in str(p) for p in result.problems))

    def test_bad_locale_and_timezone(self):
        profile = base_profile()
        profile["system_locale"] = "nope"
        profile["timezone"] = "../etc"
        result = cv.validate_profile(profile)
        self.assertGreaterEqual(len([p for p in result.problems if p.code == "invalid_config"]), 1)

    def test_linux_username_rules(self):
        profile = base_profile()
        profile["users"] = ["Alice"]
        result = cv.validate_profile(profile)
        self.assertFalse(result)

    def test_btrfs_and_systemd_boot_are_accepted(self):
        profile = base_profile()
        profile["filesystem"] = "btrfs"
        profile["bootloader"] = "systemd-boot"
        self.assertTrue(cv.validate_profile(profile))


class TestPackageRecommendation(unittest.TestCase):
    def test_core_packages_always_present(self):
        rec = package_recommend.recommend(desktop="console", audio="none")
        self.assertIn("base", rec.packages)
        self.assertIn("linux", rec.packages)

    def test_vietnamese_suggests_ibus_but_no_aur_by_default(self):
        rec = package_recommend.recommend(language="vi", use_aur=False)
        self.assertIn("ibus", rec.packages)
        self.assertEqual(rec.aur, [])

    def test_aur_requires_an_opt_in(self):
        rec = package_recommend.recommend(language="vi", use_aur=True)
        self.assertIn("ibus-bamboo", rec.aur)
        self.assertTrue(any("AUR" in w for w in rec.warnings))

    def test_pipewire_and_pulseaudio_do_not_collide(self):
        rec = package_recommend.recommend(audio="pipewire")
        self.assertNotIn("pulseaudio", rec.packages)
        self.assertEqual(rec.warnings, [])

    def test_desktop_package_sets_differ(self):
        plasma = package_recommend.recommend(desktop="plasma").packages
        sway = package_recommend.recommend(desktop="sway").packages
        self.assertNotEqual(plasma, sway)

    def test_duplicate_packages_are_removed(self):
        rec = package_recommend.recommend(extra_groups=["monitoring", "monitoring"])
        self.assertEqual(len(rec.packages), len(set(rec.packages)))


class TestHardware(unittest.TestCase):
    def test_zram_suggestion_matches_the_rust_rule(self):
        # 8 GiB -> 4096 MiB, rounded to a multiple of 128
        self.assertEqual(hw_info.suggested_zram_mb(8 << 30), 4096)
        # tiny RAM -> floor of 256 MiB
        self.assertEqual(hw_info.suggested_zram_mb(512 << 20), 256)

    def test_firmware_detection_returns_a_known_value(self):
        self.assertIn(hw_info.detect_firmware(), {"uefi", "bios", "unknown"})

    def test_ram_detection_is_plausible_on_linux(self):
        ram = hw_info.detect_ram_bytes()
        if ram:
            self.assertGreater(ram, 64 << 20)

    def test_cpu_hint_is_known(self):
        self.assertIn(hw_info.detect_cpu().microcode_hint, {"intel", "amd", "unknown"})


if __name__ == "__main__":
    unittest.main()
