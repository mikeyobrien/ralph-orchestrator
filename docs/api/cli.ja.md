# CLI API リファレンス

## 概要

CLI API は、コマンド・引数・シェル統合を含む Ralph Orchestrator のコマンドラインインター
フェースを提供します。

## メインの CLI インターフェース

### RalphCLI クラス

```python
class RalphCLI:
    """
    Ralph Orchestrator のメイン CLI インターフェース。

    Example:
        cli = RalphCLI()
        cli.run(sys.argv[1:])
    """

    def __init__(self):
        """コマンドレジストリで CLI を初期化する。"""
        self.commands = {
            'run': self.cmd_run,
            'init': self.cmd_init,
            'status': self.cmd_status,
            'clean': self.cmd_clean,
            'config': self.cmd_config,
            'agents': self.cmd_agents,
            'metrics': self.cmd_metrics,
            'checkpoint': self.cmd_checkpoint,
            'rollback': self.cmd_rollback,
            'help': self.cmd_help
        }
        self.parser = self.create_parser()

    def create_parser(self) -> argparse.ArgumentParser:
        """
        引数パーサーを作成する。

        Returns:
            ArgumentParser: 構成済みのパーサー
        """
        parser = argparse.ArgumentParser(
            prog='ralph',
            description='Ralph Orchestrator - AI task automation',
            formatter_class=argparse.RawDescriptionHelpFormatter,
            epilog="""
Examples:
  ralph run                    # 自動検出されたエージェントで実行する
  ralph run -a claude          # Claude で実行する
  ralph run -a acp             # ACP エージェントで実行する
  ralph run -a acp --acp-agent gemini --acp-permission-mode auto_approve
  ralph status                 # 現在の状態を確認する
  ralph clean                  # ワークスペースをクリーンにする
  ralph init                   # 新しいプロジェクトを初期化する
            """
        )

        # グローバル引数
        parser.add_argument(
            '--version',
            action='version',
            version='%(prog)s 1.0.0'
        )

        parser.add_argument(
            '--verbose', '-v',
            action='store_true',
            help='詳細出力を有効にする'
        )

        parser.add_argument(
            '--config', '-c',
            help='設定ファイルのパス'
        )

        # サブコマンド
        subparsers = parser.add_subparsers(
            dest='command',
            help='利用可能なコマンド'
        )

        # run コマンド
        run_parser = subparsers.add_parser(
            'run',
            help='オーケストレータを実行する'
        )
        run_parser.add_argument(
            '--agent', '-a',
            choices=['claude', 'q', 'gemini', 'acp', 'auto'],
            default='auto',
            help='使用する AI エージェント'
        )
        run_parser.add_argument(
            '--acp-agent',
            default='gemini',
            help='ACP エージェントコマンド（-a acp 用）'
        )
        run_parser.add_argument(
            '--acp-permission-mode',
            choices=['auto_approve', 'deny_all', 'allowlist', 'interactive'],
            default='auto_approve',
            help='ACP エージェントの権限処理モード'
        )
        run_parser.add_argument(
            '--prompt', '-p',
            default='PROMPT.md',
            help='プロンプトファイルのパス'
        )
        run_parser.add_argument(
            '--max-iterations', '-i',
            type=int,
            default=100,
            help='最大イテレーション数'
        )
        run_parser.add_argument(
            '--dry-run',
            action='store_true',
            help='実行しないテストモード'
        )

        # init コマンド
        subparsers.add_parser(
            'init',
            help='新しいプロジェクトを初期化する'
        )

        # status コマンド
        subparsers.add_parser(
            'status',
            help='現在の状態を表示する'
        )

        # clean コマンド
        subparsers.add_parser(
            'clean',
            help='ワークスペースをクリーンにする'
        )

        # config コマンド
        config_parser = subparsers.add_parser(
            'config',
            help='設定を管理する'
        )
        config_parser.add_argument(
            'action',
            choices=['show', 'set', 'get'],
            help='設定アクション'
        )
        config_parser.add_argument(
            'key',
            nargs='?',
            help='設定キー'
        )
        config_parser.add_argument(
            'value',
            nargs='?',
            help='設定値'
        )

        # agents コマンド
        subparsers.add_parser(
            'agents',
            help='利用可能なエージェントを一覧する'
        )

        # metrics コマンド
        metrics_parser = subparsers.add_parser(
            'metrics',
            help='メトリクスを表示する'
        )
        metrics_parser.add_argument(
            '--format',
            choices=['text', 'json', 'csv'],
            default='text',
            help='出力形式'
        )

        # checkpoint コマンド
        checkpoint_parser = subparsers.add_parser(
            'checkpoint',
            help='チェックポイントを作成する'
        )
        checkpoint_parser.add_argument(
            '--message', '-m',
            help='チェックポイントのメッセージ'
        )

        # rollback コマンド
        rollback_parser = subparsers.add_parser(
            'rollback',
            help='チェックポイントにロールバックする'
        )
        rollback_parser.add_argument(
            'checkpoint',
            nargs='?',
            help='チェックポイント ID または "last"'
        )

        return parser

    def run(self, args: List[str] = None):
        """
        引数を使って CLI を実行する。

        Args:
            args (list): コマンドライン引数

        Returns:
            int: 終了コード

        Example:
            cli = RalphCLI()
            exit_code = cli.run(['run', '--agent', 'claude'])
        """
        args = self.parser.parse_args(args)

        # ロギングをセットアップする
        if args.verbose:
            logging.basicConfig(level=logging.DEBUG)
        else:
            logging.basicConfig(level=logging.INFO)

        # 設定を読み込む
        if args.config:
            config = load_config(args.config)
        else:
            config = load_config()

        # コマンドを実行する
        if args.command:
            command = self.commands.get(args.command)
            if command:
                return command(args, config)
            else:
                print(f"Unknown command: {args.command}")
                return 1
        else:
            self.parser.print_help()
            return 0
```

## コマンドの実装

### Run コマンド

```python
def cmd_run(self, args, config):
    """
    run コマンドを実行する。

    Args:
        args: パース済みの引数
        config: 設定ディクショナリ

    Returns:
        int: 終了コード

    Example:
        cli.cmd_run(args, config)
    """
    # CLI 引数で設定を更新する
    if args.agent:
        config['agent'] = args.agent
    if args.prompt:
        config['prompt_file'] = args.prompt
    if args.max_iterations:
        config['max_iterations'] = args.max_iterations
    if args.dry_run:
        config['dry_run'] = True

    # オーケストレータを作成して実行する
    orchestrator = RalphOrchestrator(config)

    try:
        result = orchestrator.run()

        if result['success']:
            print(f"✓ Task completed in {result['iterations']} iterations")
            return 0
        else:
            print(f"✗ Task failed: {result.get('error', 'Unknown error')}")
            return 1

    except KeyboardInterrupt:
        print("\n⚠ Interrupted by user")
        return 130
    except Exception as e:
        print(f"✗ Error: {str(e)}")
        return 1
```

### Init コマンド

```python
def cmd_init(self, args, config):
    """
    新しい Ralph プロジェクトを初期化する。

    Args:
        args: パース済みの引数
        config: 設定ディクショナリ

    Returns:
        int: 終了コード

    Example:
        cli.cmd_init(args, config)
    """
    print("Initializing Ralph Orchestrator project...")

    # ディレクトリを作成する
    directories = ['.agent', '.agent/metrics', '.agent/prompts', 
                  '.agent/checkpoints', '.agent/plans']
    for directory in directories:
        os.makedirs(directory, exist_ok=True)
        print(f"  ✓ Created {directory}")

    # 既定の PROMPT.md を作成する
    if not os.path.exists('PROMPT.md'):
        with open('PROMPT.md', 'w') as f:
            f.write("""# Task Description

Describe your task here...

## Requirements
- [ ] Requirement 1
- [ ] Requirement 2

## Success Criteria
- The task is complete when...

<!-- Ralph will continue iterating until limits are reached -->
""")
        print("  ✓ Created PROMPT.md template")

    # 既定の設定を作成する
    if not os.path.exists('ralph.json'):
        with open('ralph.json', 'w') as f:
            json.dump({
                'agent': 'auto',
                'max_iterations': 100,
                'checkpoint_interval': 5
            }, f, indent=2)
        print("  ✓ Created ralph.json config")

    # 存在しなければ Git を初期化する
    if not os.path.exists('.git'):
        subprocess.run(['git', 'init'], capture_output=True)
        print("  ✓ Initialized Git repository")

    print("\n✓ Project initialized successfully!")
    print("\nNext steps:")
    print("  1. Edit PROMPT.md with your task")
    print("  2. Run: ralph run")

    return 0
```

### Status コマンド

```python
def cmd_status(self, args, config):
    """
    現在の Ralph の状態を表示する。

    Args:
        args: パース済みの引数
        config: 設定ディクショナリ

    Returns:
        int: 終了コード

    Example:
        cli.cmd_status(args, config)
    """
    print("Ralph Orchestrator Status")
    print("=" * 40)

    # プロンプトファイルを確認する
    if os.path.exists('PROMPT.md'):
        print(f"✓ Prompt: PROMPT.md exists")

        # タスクが完了しているか確認する
        with open('PROMPT.md') as f:
            content = f.read()
        # レガシーの完了確認 - 現在は使われていない
        # if 'TASK_COMPLETE' in content:
            print("✓ Status: COMPLETE")
        else:
            print("⚠ Status: IN PROGRESS")
    else:
        print("✗ Prompt: PROMPT.md not found")

    # 状態を確認する
    state_file = '.agent/metrics/state_latest.json'
    if os.path.exists(state_file):
        with open(state_file) as f:
            state = json.load(f)

        print(f"\nLatest State:")
        print(f"  Iterations: {state.get('iteration_count', 0)}")
        print(f"  Runtime: {state.get('runtime', 0):.1f}s")
        print(f"  Agent: {state.get('agent', 'none')}")
        print(f"  Errors: {len(state.get('errors', []))}")

    # 利用可能なエージェントを確認する
    manager = AgentManager()
    available = manager.detect_available_agents()
    print(f"\nAvailable Agents: {', '.join(available) if available else 'none'}")

    # Git の状態を確認する
    result = subprocess.run(
        ['git', 'status', '--porcelain'],
        capture_output=True,
        text=True
    )
    if result.stdout:
        print(f"\n⚠ Uncommitted changes present")
    else:
        print(f"\n✓ Git: clean working directory")

    return 0
```

### Clean コマンド

```python
def cmd_clean(self, args, config):
    """
    Ralph ワークスペースをクリーンにする。

    Args:
        args: パース済みの引数
        config: 設定ディクショナリ

    Returns:
        int: 終了コード

    Example:
        cli.cmd_clean(args, config)
    """
    print("Cleaning Ralph workspace...")

    # クリーンにする前に確認する
    response = input("This will remove all Ralph data. Continue? [y/N]: ")
    if response.lower() != 'y':
        print("Cancelled")
        return 0

    # ディレクトリをクリーンにする
    directories = [
        '.agent/metrics',
        '.agent/prompts',
        '.agent/checkpoints',
        '.agent/logs'
    ]

    for directory in directories:
        if os.path.exists(directory):
            shutil.rmtree(directory)
            os.makedirs(directory)
            print(f"  ✓ Cleaned {directory}")

    # 状態をリセットする
    state = StateManager()
    state.reset()
    print("  ✓ Reset state")

    print("\n✓ Workspace cleaned successfully!")

    return 0
```

## シェル統合

### Bash 補完

```bash
# ralph-completion.bash
_ralph_completion() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    # メインコマンド
    opts="run init status clean config agents metrics checkpoint rollback help"

    case "${prev}" in
        ralph)
            COMPREPLY=( $(compgen -W "${opts}" -- ${cur}) )
            return 0
            ;;
        --agent|-a)
            COMPREPLY=( $(compgen -W "claude q gemini acp auto" -- ${cur}) )
            return 0
            ;;
        --acp-agent)
            COMPREPLY=( $(compgen -c -- ${cur}) )
            return 0
            ;;
        --acp-permission-mode)
            COMPREPLY=( $(compgen -W "auto_approve deny_all allowlist interactive" -- ${cur}) )
            return 0
            ;;
        --format)
            COMPREPLY=( $(compgen -W "text json csv" -- ${cur}) )
            return 0
            ;;
        config)
            COMPREPLY=( $(compgen -W "show set get" -- ${cur}) )
            return 0
            ;;
    esac

    # プロンプトファイル用のファイル補完
    if [[ ${cur} == *.md ]]; then
        COMPREPLY=( $(compgen -f -X '!*.md' -- ${cur}) )
        return 0
    fi

    COMPREPLY=( $(compgen -W "${opts}" -- ${cur}) )
}

complete -F _ralph_completion ralph
```

### ZSH 補完

```zsh
# ralph-completion.zsh
#compdef ralph

_ralph() {
    local -a commands
    commands=(
        'run:オーケストレータを実行する'
        'init:プロジェクトを初期化する'
        'status:状態を表示する'
        'clean:ワークスペースをクリーンにする'
        'config:設定を管理する'
        'agents:エージェントを一覧する'
        'metrics:メトリクスを表示する'
        'checkpoint:チェックポイントを作成する'
        'rollback:チェックポイントにロールバックする'
        'help:ヘルプを表示する'
    )

    _arguments \
        '--version[バージョンを表示する]' \
        '--verbose[詳細出力を有効にする]' \
        '--config[設定ファイル]:file:_files' \
        '1:command:->command' \
        '*::arg:->args'

    case $state in
        command)
            _describe 'command' commands
            ;;
        args)
            case $words[1] in
                run)
                    _arguments \
                        '--agent[AI エージェント]:agent:(claude q gemini acp auto)' \
                        '--prompt[プロンプトファイル]:file:_files -g "*.md"' \
                        '--max-iterations[最大イテレーション]:number' \
                        '--acp-agent[ACP エージェントコマンド]:command' \
                        '--acp-permission-mode[権限モード]:mode:(auto_approve deny_all allowlist interactive)' \
                        '--dry-run[テストモード]'
                    ;;
                config)
                    _arguments \
                        '1:action:(show set get)' \
                        '2:key' \
                        '3:value'
                    ;;
            esac
            ;;
    esac
}
```

## 対話モード

```python
class InteractiveCLI:
    """
    Ralph の対話的 CLI モード。

    Example:
        interactive = InteractiveCLI()
        interactive.run()
    """

    def __init__(self):
        self.running = True
        self.orchestrator = None
        self.config = load_config()

    def run(self):
        """対話モードを実行する。"""
        print("Ralph Orchestrator Interactive Mode")
        print("Type 'help' for commands, 'exit' to quit")
        print()

        while self.running:
            try:
                command = input("ralph> ").strip()
                if command:
                    self.execute_command(command)
            except KeyboardInterrupt:
                print("\nUse 'exit' to quit")
            except EOFError:
                self.running = False

    def execute_command(self, command: str):
        """対話コマンドを実行する。"""
        parts = command.split()
        cmd = parts[0]
        args = parts[1:] if len(parts) > 1 else []

        commands = {
            'help': self.cmd_help,
            'run': self.cmd_run,
            'status': self.cmd_status,
            'stop': self.cmd_stop,
            'config': self.cmd_config,
            'agents': self.cmd_agents,
            'exit': self.cmd_exit,
            'quit': self.cmd_exit
        }

        if cmd in commands:
            commands[cmd](args)
        else:
            print(f"Unknown command: {cmd}")

    def cmd_help(self, args):
        """ヘルプを表示する。"""
        print("""
Available commands:
  run [agent]    - オーケストレータを開始する
  status         - 現在の状態を表示する
  stop           - オーケストレータを停止する
  config [key]   - 設定を表示/設定する
  agents         - 利用可能なエージェントを一覧する
  help           - このヘルプを表示する
  exit           - 対話モードを終了する
        """)

    def cmd_exit(self, args):
        """対話モードを終了する。"""
        if self.orchestrator:
            print("Stopping orchestrator...")
            # オーケストレータを停止する
        print("Goodbye!")
        self.running = False
```

## プラグインシステム

```python
class CLIPlugin:
    """
    CLI プラグインの基底クラス。

    Example:
        class MyPlugin(CLIPlugin):
            def register_commands(self, cli):
                cli.add_command('mycommand', self.my_command)
    """

    def __init__(self, name: str):
        self.name = name

    def register_commands(self, cli: RalphCLI):
        """CLI にプラグインのコマンドを登録する。"""
        raise NotImplementedError

    def register_arguments(self, parser: argparse.ArgumentParser):
        """プラグインの引数を登録する。"""
        pass

class PluginManager:
    """CLI プラグインを管理する。"""

    def __init__(self):
        self.plugins = []

    def load_plugin(self, plugin: CLIPlugin):
        """プラグインを読み込む。"""
        self.plugins.append(plugin)

    def register_all(self, cli: RalphCLI):
        """すべてのプラグインを CLI に登録する。"""
        for plugin in self.plugins:
            plugin.register_commands(cli)
```
