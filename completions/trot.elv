
use builtin;
use str;

set edit:completion:arg-completer[trot] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'trot'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'trot'= {
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand daemon 'Run the tracking daemon (talks to the treadmill, serves the API)'
            cand today 'Today''s totals'
            cand status 'Whether the daemon is up and a treadmill is connected'
            cand log 'Recent sessions'
            cand scan 'Scan for nearby treadmills and pick one to pair (interactive)'
            cand devices 'List paired treadmills (the active one is marked with *)'
            cand pair 'Pair a treadmill from `trot scan` and make it the active one'
            cand unpair 'Forget the active treadmill'
            cand completions 'Print (or install) a shell completion script'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'trot;daemon'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;today'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;status'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;log'= {
            cand --limit 'How many to show'
            cand --week 'Only sessions from the last 7 days'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;scan'= {
            cand --seconds 'How long to scan for, in seconds (1–15)'
            cand --all 'Show every Bluetooth device, not just treadmills'
            cand --list 'Just print the list; don''t show the interactive picker'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;devices'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;pair'= {
            cand --name 'A friendly name to remember it by'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;unpair'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'trot;completions'= {
            cand --install 'Write it where the shell will find it, instead of printing to stdout'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'trot;help'= {
            cand daemon 'Run the tracking daemon (talks to the treadmill, serves the API)'
            cand today 'Today''s totals'
            cand status 'Whether the daemon is up and a treadmill is connected'
            cand log 'Recent sessions'
            cand scan 'Scan for nearby treadmills and pick one to pair (interactive)'
            cand devices 'List paired treadmills (the active one is marked with *)'
            cand pair 'Pair a treadmill from `trot scan` and make it the active one'
            cand unpair 'Forget the active treadmill'
            cand completions 'Print (or install) a shell completion script'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'trot;help;daemon'= {
        }
        &'trot;help;today'= {
        }
        &'trot;help;status'= {
        }
        &'trot;help;log'= {
        }
        &'trot;help;scan'= {
        }
        &'trot;help;devices'= {
        }
        &'trot;help;pair'= {
        }
        &'trot;help;unpair'= {
        }
        &'trot;help;completions'= {
        }
        &'trot;help;help'= {
        }
    ]
    $completions[$command]
}
