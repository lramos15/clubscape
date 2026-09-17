import importlib.util
import os
import stat
import tempfile
import unittest
from unittest import mock
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("dev", ROOT / "tools/dev.py")
dev = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dev)


class DevelopmentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)

    def test_secrets_are_private_random_and_persist_without_replacement(self):
        first_path = self.root / "first"
        first = dev.write_secret(first_path)
        second = dev.write_secret(self.root / "second")
        self.assertEqual(len(first), 43)
        self.assertNotEqual(first, second)
        self.assertEqual(dev.write_secret(first_path), first)
        self.assertEqual(stat.S_IMODE(first_path.stat().st_mode), 0o600)

    def test_unsafe_existing_secret_or_directory_is_rejected(self):
        path = self.root / "secret"
        dev.write_secret(path)
        path.chmod(0o644)
        with self.assertRaises(dev.DevelopmentError):
            dev.write_secret(path)
        path.chmod(0o600)
        path.write_text("not-valid")
        with self.assertRaises(dev.DevelopmentError):
            dev.write_secret(path)
        link = self.root / "link"
        link.symlink_to(path)
        with self.assertRaises(dev.DevelopmentError):
            dev.write_secret(link)
        public = self.root / "public"
        public.mkdir()
        public.chmod(0o755)
        with self.assertRaises(dev.DevelopmentError):
            dev.private_directory(public)

    def test_database_destinations_and_ports_are_bounded(self):
        url = dev.database_url("a:b@c", 55432, "clubscape")
        self.assertIn("a%3Ab%40c@127.0.0.1:55432/clubscape", url)
        for port in (0, 65536, -1, True, "5432"):
            with self.assertRaises(dev.DevelopmentError):
                dev.database_url("secret", port, "clubscape")
        with self.assertRaises(dev.DevelopmentError):
            dev.database_url("secret", 5432, "someones_live_database")

    def test_validation_workspaces_cannot_escape_the_repository(self):
        self.assertEqual(dev.workspace_path(ROOT), ROOT)
        with self.assertRaises(dev.DevelopmentError):
            dev.workspace_path(self.root)

    def test_command_failure_redacts_database_credentials(self):
        secret = "postgresql://user:secret@127.0.0.1/database"
        env = dict(os.environ, DATABASE_URL=secret)
        with self.assertRaises(dev.DevelopmentError) as error:
            dev.run(
                ["python3", "-c", "import os; print(os.environ['DATABASE_URL']); raise SystemExit(1)"],
                env=env,
            )
        self.assertNotIn(secret, str(error.exception))
        self.assertIn("redacted", str(error.exception))

    def test_report_paths_cannot_escape_the_repository(self):
        with self.assertRaises(dev.DevelopmentError):
            dev.write_report("../outside.json", {"result": "passed"})

    def test_adversarial_fixtures_are_isolated_from_the_independent_client(self):
        container = "clubscape-m1-test-" + "a" * 16
        with mock.patch.object(dev, "run") as run:
            dev.reset_test_database(container)
            self.assertEqual(run.call_args_list, [
                mock.call(["docker", "exec", container, "dropdb", "--username=clubscape", "clubscape_m1_test"]),
                mock.call(["docker", "exec", container, "createdb", "--username=clubscape", "clubscape_m1_test"]),
            ])
        for foreign in ("postgres", "clubscape-m1-postgres-1", "clubscape-m1-test-invalid"):
            with mock.patch.object(dev, "run") as run:
                with self.assertRaises(dev.DevelopmentError):
                    dev.reset_test_database(foreign)
                run.assert_not_called()

    def test_failed_server_exit_cannot_be_reported_as_successful_cleanup(self):
        process = mock.Mock()
        process.poll.return_value = 1
        process.returncode = 1
        with self.assertRaisesRegex(dev.DevelopmentError, "exited unsuccessfully"):
            dev.stop_process(process)
        process.terminate.assert_not_called()
        process.poll.return_value = None
        process.returncode = 2
        with self.assertRaisesRegex(dev.DevelopmentError, "exited unsuccessfully"):
            dev.stop_process(process)
        process.terminate.assert_called_once()
        process.wait.assert_called_once_with(timeout=15)


if __name__ == "__main__":
    unittest.main()
