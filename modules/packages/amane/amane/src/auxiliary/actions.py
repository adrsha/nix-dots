"""Action worker. All user strings are argv, never shell commands.
No capture/power operation runs merely by opening an island.
"""
import json, os, tempfile, pathlib, shutil, signal, subprocess, sys, time, urllib.parse, threading, shlex

def tool(name):
    path = shutil.which(name)
    if not path:
        raise RuntimeError(f'{name} is not installed; rebuild the Amane Nix module first.')
    return path

class Cancelled(Exception):
    pass

CURRENT = None
CANCELLED = False

def on_cancel(*_):
    # SIGTERM from the shell: stop whatever is waiting (usually slurp) and leave.
    global CANCELLED
    CANCELLED = True
    if CURRENT is not None and CURRENT.poll() is None:
        CURRENT.terminate()

def run(args, input=None, picking=False):
    """Runs a tool. stdin is never inherited: the shell keeps our stdin open as a
    control pipe, and slurp would wait on it forever to read predefined regions."""
    global CURRENT
    CURRENT = subprocess.Popen(args, stdin=subprocess.PIPE if input is not None else subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    out, err = CURRENT.communicate(input)
    code, CURRENT = CURRENT.returncode, None
    if CANCELLED:
        raise Cancelled()
    if code:
        msg = err.decode(errors='replace').strip()
        if picking and (not msg or 'cancel' in msg.lower()):
            raise Cancelled()
        raise RuntimeError(msg or f'{args[0]} failed')
    return out

def copy(path):
    # wl-copy forks a background clipboard server. It must not inherit our pipes,
    # or this call waits until the clipboard is overwritten and the UI never gets FILE.
    with open(path, 'rb') as data:
        result = subprocess.run([tool('wl-copy'), '--type', 'image/png'], stdin=data,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if result.returncode:
        raise RuntimeError('Could not copy image to clipboard')

def destination(record=False):
    d = pathlib.Path.home() / ('Videos/recordings' if record else 'Pictures/screenshots')
    d.mkdir(parents=True, exist_ok=True)
    return d / (time.strftime('%Y-%m-%d_%H-%M-%S') + f'_{time.time_ns()%1000000:06d}' + ('.mp4' if record else '.png'))

def scratch():
    # Unsaved captures live here until Save/Annotate; cancelling deletes them.
    base = pathlib.Path(os.environ.get('XDG_RUNTIME_DIR') or tempfile.gettempdir())
    return base / f'amane-shot-{time.time_ns()}.png'

def slurp_style(color):
    c = color.lstrip('#')[:6]
    # Fully transparent background, selection and option boxes: only the border
    # (in the theme colour) is drawn, so the screen stays visible underneath.
    return ['-b', '#00000000', '-s', '#00000000', '-B', '#00000000', '-c', f'#{c}ff', '-w', '3']

def geometry(kind, color):
    if kind == 'region':
        out = run([tool('slurp')] + slurp_style(color), picking=True).decode().strip()
        print('SELECTED', flush=True)
        return out
    if kind == 'window':
        # Real visible client rectangles, supplied to slurp's rectangle mode.
        snapshot = json.loads(run([tool('mmsg'), 'get', 'all-clients']))
        rows = []
        for c in snapshot.get('clients', []):
            if not c.get('is_visible') or c.get('is_minimized') or c.get('is_swallowedby'):
                continue
            x,y,w,h = (int(c.get(k,0)) for k in ('x','y','width','height'))
            if w>0 and h>0:
                rows.append(f'{x},{y} {w}x{h}')
        if not rows:
            raise RuntimeError('No visible Mango windows available for capture.')
        out = run([tool('slurp'), '-r'] + slurp_style(color), input=('\n'.join(rows)+'\n').encode(), picking=True).decode().strip()
        print('SELECTED', flush=True)
        return out
    if kind != 'full':
        raise RuntimeError('Unknown capture type')
    return None

def settle(started, minimum):
    # Let the island finish leaving, the border fade back and slurp unmap,
    # so none of them end up in the capture.
    time.sleep(max(0.0, minimum - (time.time() - started)))

def main():
    action, *args = sys.argv[1:]
    signal.signal(signal.SIGTERM, on_cancel)
    signal.signal(signal.SIGINT, on_cancel)
    started = time.time()
    # The capture colour always comes from the shell's sampled theme.
    color = args[1] if len(args) > 1 else None
    if action in ('screenshot', 'record') and not color:
        raise RuntimeError('No theme colour supplied')
    if action == 'screenshot':
        tool('grim');tool('wl-copy')
        if args[0] != 'full': tool('slurp')
        rect = geometry(args[0], color)
        settle(started, 1.0 if rect is None else 0.45)
        path=scratch()
        cmd=[tool('grim')]
        if rect: cmd += ['-g', rect]
        cmd += [str(path)]
        run(cmd);print('FILE\t'+str(path), flush=True)
    elif action == 'save':
        tool('wl-copy')
        original=pathlib.Path(args[0]);target=destination()
        shutil.move(str(original),str(target));copy(target)
        print('DONE\t'+str(target), flush=True)
    elif action == 'edit':
        original=pathlib.Path(args[0]); output=destination()
        # Satty copies and saves when the user confirms. Closing it without
        # confirming leaves no output: that is a cancel, nothing is kept.
        try:
            run([tool('satty'),'--filename',str(original),'--output-filename',str(output),
                 '--copy-command',tool('wl-copy'),'--save-after-copy','--early-exit'])
        finally:
            original.unlink(missing_ok=True)
        if not output.exists():
            raise Cancelled()
        copy(output)
        print('DONE\t'+str(output), flush=True)
    elif action == 'record':
        tool('wf-recorder')
        if args[0] != 'full': tool('slurp')
        rect=geometry(args[0], color)
        settle(started, 1.0 if rect is None else 0.45)
        path=destination(True)
        cmd=[tool('wf-recorder'),'-f',str(path)]
        if rect:cmd+=['-g',rect]
        child=subprocess.Popen(cmd,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        def stop(*_):
            if child.poll() is None:child.send_signal(signal.SIGINT)
        signal.signal(signal.SIGINT,stop);signal.signal(signal.SIGTERM,stop)
        # The parent owns this worker's stdin; EOF on shell exit/restart also
        # stops this exact recorder. No PID files and no killing unrelated jobs.
        def control():
            sys.stdin.readline()
            stop()
        threading.Thread(target=control,daemon=True).start()
        print('RECORDING\t'+str(path),flush=True)
        _,err=child.communicate()
        if child.returncode not in (0, -signal.SIGINT):
            raise RuntimeError(err.decode(errors='replace').strip() or 'Recording failed')
        print('DONE\t'+str(path),flush=True)
    elif action == 'browser':
        text=args[0].strip()
        if not text:raise RuntimeError('Enter a search or URL after s ')
        parsed=urllib.parse.urlsplit(text)
        if parsed.scheme in ('https','http'):url=text
        elif '.' in text and ' ' not in text:url='https://'+text
        else:url='https://www.google.com/search?q='+urllib.parse.quote(text)
        run([tool('xdg-open'),url]);print('DONE\tOpened browser',flush=True)
    elif action == 'launch':
        # freedesktop Exec uses quoted argv, not shell evaluation. The library
        # already removed field codes, so no query string ever becomes code.
        argv=shlex.split(args[0])
        if not argv:raise RuntimeError('Empty desktop command')
        subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
        print('DONE\tApplication launched',flush=True)
    elif action == 'session':
        name=args[0]
        if name=='logout':cmd=[tool('mmsg'),'dispatch','quit']
        elif name in ('restart','shutdown','sleep'):
            cmd=[tool('systemctl'),{'restart':'reboot','shutdown':'poweroff','sleep':'suspend'}[name]]
        elif name=='lock':
            text=json.loads((pathlib.Path.home()/'.local/state/theme/active.json').read_text())
            colors=text.get('colors',{})
            color=lambda key,fallback:colors.get(key,fallback).lstrip('#')[:6]
            cmd=[tool('swaylock'),'--color',color('BASE00','#000000'),'--indicator-radius','64',
                 '--ring-color',color('BLUE','#81a1c1'),'--inside-color',color('BASE00','#000000'),
                 '--line-color',color('BASE00','#000000'),'--text-color',color('WHITE','#d8dee9')]
        else:raise RuntimeError('Unknown session action')
        run(cmd);print('DONE\t'+name,flush=True)

if __name__=='__main__':
    try:main()
    except Cancelled:
        print('CANCEL', flush=True)
    except Exception as e:
        print('ERROR\t'+str(e).replace('\n',' ')[:350],flush=True)
        sys.exit(1)
