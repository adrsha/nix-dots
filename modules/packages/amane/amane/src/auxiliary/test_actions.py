"""Safe action-contract tests: every external program is mocked."""
import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('actions',pathlib.Path(__file__).with_name('actions.py'))
a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a)

class Actions(unittest.TestCase):
    def invoke(self,args):
        with patch.object(sys,'argv',['actions']+args):a.main()
    def test_window_capture_uses_real_visible_rectangles(self):
        data=b'{"clients":[{"is_visible":true,"x":-100,"y":20,"width":800,"height":600},{"is_visible":false,"width":4,"height":5}]}'
        with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a,'run',side_effect=[data,b'-100,20 800x600'])as run:
            self.assertEqual(a.geometry('window','#ffd866'),'-100,20 800x600')
            self.assertEqual(run.call_args.kwargs['input'],b'-100,20 800x600\n')
    def test_region_slurp_never_inherits_stdin(self):
        # An open stdin pipe made slurp wait for predefined regions forever.
        proc=unittest.mock.MagicMock();proc.communicate.return_value=(b'1,2 3x4\n',b'');proc.returncode=0
        with patch.object(a.subprocess,'Popen',return_value=proc)as popen:
            a.run(['slurp'])
            self.assertIs(popen.call_args.kwargs['stdin'],a.subprocess.DEVNULL)
    def test_cancelled_selection_is_not_an_error(self):
        proc=unittest.mock.MagicMock();proc.communicate.return_value=(b'',b'selection cancelled');proc.returncode=1
        with patch.object(a.subprocess,'Popen',return_value=proc):
            with self.assertRaises(a.Cancelled):a.run(['slurp'],picking=True)
    def test_cancelled_pick_propagates_to_the_cancel_handler(self):
        with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a,'geometry',side_effect=a.Cancelled()):
            with self.assertRaises(a.Cancelled):self.invoke(['screenshot','region','#ffd866'])
    def test_search_is_encoded_and_never_shell(self):
        with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a,'run')as run:
            self.invoke(['browser','hello & $(unsafe)'])
            argv=run.call_args.args[0]
            self.assertEqual(argv[0],'xdg-open')
            self.assertIn('hello%20%26%20%24%28unsafe%29',argv[1])
    def test_url_is_preserved(self):
        with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a,'run')as run:
            self.invoke(['browser','https://example.org/a?q=b'])
            self.assertEqual(run.call_args.args[0],['xdg-open','https://example.org/a?q=b'])
    def test_screenshot_goes_to_scratch_not_pictures_or_clipboard(self):
        with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a.time,'sleep'),patch.object(a,'scratch',return_value=pathlib.Path('/tmp/amane-shot-1.png')),patch.object(a,'geometry',return_value='10,20 300x400'),patch.object(a,'run')as run,patch.object(a,'copy')as copy:
            self.invoke(['screenshot','region','#aabbcc'])
            self.assertEqual(run.call_args.args[0],['grim','-g','10,20 300x400','/tmp/amane-shot-1.png'])
            copy.assert_not_called()
    def test_screenshot_requires_a_theme_colour(self):
        with self.assertRaisesRegex(RuntimeError,'theme colour'):self.invoke(['screenshot','region'])
    def test_slurp_draws_only_a_coloured_border(self):
        style=a.slurp_style('#112233')
        self.assertEqual(style[style.index('-s')+1],'#00000000')
        self.assertEqual(style[style.index('-B')+1],'#00000000')
        self.assertEqual(style[style.index('-c')+1],'#112233ff')
    def test_save_moves_capture_and_copies(self):
        with tempfile.TemporaryDirectory()as d:
            src=pathlib.Path(d)/'amane-shot-1.png';src.write_bytes(b'x');dest=pathlib.Path(d)/'out.png'
            with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a,'destination',return_value=dest),patch.object(a,'copy')as copy:
                self.invoke(['save',str(src)])
            self.assertTrue(dest.exists() and not src.exists());copy.assert_called_once_with(dest)
    def test_edit_saves_copies_and_removes_scratch(self):
        with tempfile.TemporaryDirectory()as d:
            original=pathlib.Path(d)/'amane-shot-1.png';original.write_bytes(b'x');edited=pathlib.Path(d)/'out.png'
            def satty(*_a,**_k):edited.write_bytes(b'edited')
            with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a,'destination',return_value=edited),patch.object(a,'run',side_effect=satty)as run,patch.object(a,'copy')as copy:
                self.invoke(['edit',str(original)])
            self.assertIn('--save-after-copy',run.call_args.args[0]);copy.assert_called_once_with(edited);self.assertFalse(original.exists())
    def test_leaving_the_editor_unconfirmed_is_a_cancel(self):
        with tempfile.TemporaryDirectory()as d:
            original=pathlib.Path(d)/'amane-shot-1.png';original.write_bytes(b'x');edited=pathlib.Path(d)/'out.png'
            with patch.object(a,'tool',side_effect=lambda x:x),patch.object(a,'destination',return_value=edited),patch.object(a,'run'),patch.object(a,'copy')as copy:
                with self.assertRaises(a.Cancelled):self.invoke(['edit',str(original)])
            copy.assert_not_called();self.assertFalse(original.exists())
    def test_desktop_command_is_argv_not_shell(self):
        with patch.object(a.subprocess,'Popen')as start:
            self.invoke(['launch','example "hello world" "$HOME; echo no"'])
            self.assertEqual(start.call_args.args[0],['example','hello world','$HOME; echo no'])
            self.assertNotIn('shell',start.call_args.kwargs)
    def test_missing_tool_is_explicit(self):
        with patch.object(a.shutil,'which',return_value=None):
            with self.assertRaisesRegex(RuntimeError,'rebuild the Amane'):a.tool('missing')

if __name__=='__main__':unittest.main()
