# Building a CLI Tool with Hats

This example shows how to use Hats to create a command-line tool with argparse, subcommands, and proper packaging.

## Task Description

Create a Python CLI tool for file management with:
- Multiple subcommands
- Progress bars
- Configuration file support
- Error handling
- Installation script

## PROMPT.md File

```markdown
# Task: Build File Manager CLI Tool

Create a Python CLI tool called 'fman' with the following features:

## Commands

1. **list** - List files in directory
   - Options: --all, --size, --date
   - Show file sizes and modification dates
   
2. **search** - Search for files
   - Options: --name, --extension, --content
   - Support wildcards and regex
   
3. **copy** - Copy files/directories
   - Show progress bar for large files
   - Options: --recursive, --overwrite
   
4. **move** - Move files/directories
   - Confirm before overwriting
   - Options: --force
   
5. **delete** - Delete files/directories
   - Require confirmation
   - Options: --force, --recursive

## Requirements

- Use argparse for CLI parsing
- Use click or rich for better UX
- Progress bars with tqdm
- Colored output
- Configuration file support (~/.fmanrc)
- Comprehensive error handling
- Unit tests
- Setup.py for installation

## Project Structure

```
file-manager-cli/
├── fman/
│   ├── __init__.py
│   ├── __main__.py      # Entry point
│   ├── cli.py           # CLI interface
│   ├── commands/        # Command implementations
│   │   ├── __init__.py
│   │   ├── list_cmd.py
│   │   ├── search_cmd.py
│   │   ├── copy_cmd.py
│   │   ├── move_cmd.py
│   │   └── delete_cmd.py
│   ├── utils.py         # Utility functions
│   └── config.py        # Configuration handling
├── tests/
│   └── test_commands.py
├── setup.py
├── requirements.txt
└── README.md
```

<!-- The orchestrator will continue iterations until all requirements are met -->
```

## Running Hats

```bash
# Initialize and run
hats init
cp cli-tool-prompt.md PROMPT.md
hats run --agent claude --max-iterations 40
```

## Expected Output

### fman/cli.py

```python
#!/usr/bin/env python3
import argparse
import sys
from pathlib import Path
from rich.console import Console
from rich.table import Table

from fman.commands import (
    list_cmd, search_cmd, copy_cmd, 
    move_cmd, delete_cmd
)
from fman.config import load_config

console = Console()

def create_parser():
    """Create argument parser"""
    parser = argparse.ArgumentParser(
        prog='fman',
        description='File Manager CLI Tool',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    
    parser.add_argument(
        '--version',
        action='version',
        version='%(prog)s 1.0.0'
    )
    
    parser.add_argument(
        '--config',
        help='Config file path',
        default='~/.fmanrc'
    )
    
    subparsers = parser.add_subparsers(
        dest='command',
        help='Available commands'
    )
    
    # List command
    list_parser = subparsers.add_parser(
        'list',
        help='List files in directory'
    )
    list_parser.add_argument(
        'path',
        nargs='?',
        default='.',
        help='Directory path'
    )
    list_parser.add_argument(
        '--all', '-a',
        action='store_true',
        help='Show hidden files'
    )
    list_parser.add_argument(
        '--size', '-s',
        action='store_true',
        help='Sort by size'
    )
    list_parser.add_argument(
        '--date', '-d',
        action='store_true',
        help='Sort by date'
    )
    
    # Search command
    search_parser = subparsers.add_parser(
        'search',
        help='Search for files'
    )
    search_parser.add_argument(
        'pattern',
        help='Search pattern'
    )
    search_parser.add_argument(
        '--path', '-p',
        default='.',
        help='Search path'
    )
    search_parser.add_argument(
        '--name', '-n',
        action='store_true',
        help='Search in filenames'
    )
    search_parser.add_argument(
        '--content', '-c',
        action='store_true',
        help='Search in file contents'
    )
    search_parser.add_argument(
        '--extension', '-e',
        help='Filter by extension'
    )
    
    # Copy command
    copy_parser = subparsers.add_parser(
        'copy',
        help='Copy files or directories'
    )
    copy_parser.add_argument('source', help='Source path')
    copy_parser.add_argument('dest', help='Destination path')
    copy_parser.add_argument(
        '--recursive', '-r',
        action='store_true',
        help='Copy recursively'
    )
    copy_parser.add_argument(
        '--overwrite', '-o',
        action='store_true',
        help='Overwrite existing files'
    )
    
    # Move command
    move_parser = subparsers.add_parser(
        'move',
        help='Move files or directories'
    )
    move_parser.add_argument('source', help='Source path')
    move_parser.add_argument('dest', help='Destination path')
    move_parser.add_argument(
        '--force', '-f',
        action='store_true',
        help='Force move without confirmation'
    )
    
    # Delete command
    delete_parser = subparsers.add_parser(
        'delete',
        help='Delete files or directories'
    )
    delete_parser.add_argument(
        'path',
        nargs='+',
        help='Path(s) to delete'
    )
    delete_parser.add_argument(
        '--force', '-f',
        action='store_true',
        help='Force delete without confirmation'
    )
    delete_parser.add_argument(
        '--recursive', '-r',
        action='store_true',
        help='Delete directories recursively'
    )
    
    return parser

def main():
    """Main entry point"""
    parser = create_parser()
    args = parser.parse_args()
    
    # Load configuration
    config = load_config(args.config)
    
    # Execute command
    commands = {
        'list': list_cmd.execute,
        'search': search_cmd.execute,
        'copy': copy_cmd.execute,
        'move': move_cmd.execute,
        'delete': delete_cmd.execute
    }
    
    if args.command:
        try:
            result = commands[args.command](args, config)
            sys.exit(0 if result else 1)
        except KeyboardInterrupt:
            console.print("\n[yellow]Interrupted by user[/yellow]")
            sys.exit(130)
        except Exception as e:
            console.print(f"[red]Error: {e}[/red]")
            sys.exit(1)
    else:
        parser.print_help()
        sys.exit(0)

if __name__ == '__main__':
    main()
```

### fman/commands/list_cmd.py

```python
import os
from pathlib import Path
from datetime import datetime
from rich.console import Console
from rich.table import Table
from rich.filesize import decimal

console = Console()

def execute(args, config):
    """Execute list command"""
    path = Path(args.path).expanduser().resolve()
    
    if not path.exists():
        console.print(f"[red]Error: Path '{path}' does not exist[/red]")
        return False
    
    if not path.is_dir():
        console.print(f"[red]Error: '{path}' is not a directory[/red]")
        return False
    
    # Get files
    if args.all:
        files = list(path.iterdir())
    else:
        files = [f for f in path.iterdir() if not f.name.startswith('.')]
    
    # Sort files
    if args.size:
        files.sort(key=lambda f: f.stat().st_size if f.is_file() else 0, 
                  reverse=True)
    elif args.date:
        files.sort(key=lambda f: f.stat().st_mtime, reverse=True)
    else:
        files.sort(key=lambda f: f.name.lower())
    
    # Create table
    table = Table(title=f"Files in {path}")
    table.add_column("Name", style="cyan")
    table.add_column("Type", style="magenta")
    table.add_column("Size", justify="right", style="green")
    table.add_column("Modified", style="yellow")
    
    for file in files:
        stat = file.stat()
        
        # Determine type
        if file.is_dir():
            file_type = "DIR"
            size = "-"
        elif file.is_symlink():
            file_type = "LINK"
            size = "-"
        else:
            file_type = "FILE"
            size = decimal(stat.st_size)
        
        # Format modification time
        mtime = datetime.fromtimestamp(stat.st_mtime)
        mtime_str = mtime.strftime("%Y-%m-%d %H:%M")
        
        table.add_row(
            file.name,
            file_type,
            size,
            mtime_str
        )
    
    console.print(table)
    return True
```

### fman/commands/copy_cmd.py

```python
import shutil
from pathlib import Path
from tqdm import tqdm
from rich.console import Console

console = Console()

def copy_with_progress(src, dst):
    """Copy file with progress bar"""
    src_size = src.stat().st_size
    
    with tqdm(total=src_size, unit='B', unit_scale=True, 
              desc=f"Copying {src.name}") as pbar:
        def callback(copied, total):
            pbar.update(copied - pbar.n)
        
        # For large files, use callback
        if src_size > 1024 * 1024:  # 1MB
            with open(src, 'rb') as fsrc:
                with open(dst, 'wb') as fdst:
                    copied = 0
                    while True:
                        buf = fsrc.read(1024 * 64)  # 64KB chunks
                        if not buf:
                            break
                        fdst.write(buf)
                        copied += len(buf)
                        callback(copied, src_size)
        else:
            shutil.copy2(src, dst)
            callback(src_size, src_size)

def execute(args, config):
    """Execute copy command"""
    source = Path(args.source).expanduser().resolve()
    dest = Path(args.dest).expanduser().resolve()
    
    if not source.exists():
        console.print(f"[red]Error: Source '{source}' does not exist[/red]")
        return False
    
    # Handle directory copy
    if source.is_dir():
        if not args.recursive:
            console.print("[red]Error: Use -r to copy directories[/red]")
            return False
        
        if dest.exists() and not args.overwrite:
            response = console.input(
                f"[yellow]'{dest}' exists. Overwrite? [y/N]:[/yellow] "
            )
            if response.lower() != 'y':
                console.print("[yellow]Copy cancelled[/yellow]")
                return False
        
        console.print(f"Copying directory '{source}' to '{dest}'...")
        shutil.copytree(source, dest, dirs_exist_ok=args.overwrite)
        console.print("[green]✓ Directory copied successfully[/green]")
    
    # Handle file copy
    else:
        if dest.is_dir():
            dest = dest / source.name
        
        if dest.exists() and not args.overwrite:
            response = console.input(
                f"[yellow]'{dest}' exists. Overwrite? [y/N]:[/yellow] "
            )
            if response.lower() != 'y':
                console.print("[yellow]Copy cancelled[/yellow]")
                return False
        
        copy_with_progress(source, dest)
        console.print("[green]✓ File copied successfully[/green]")
    
    return True
```

### setup.py

```python
from setuptools import setup, find_packages

with open("README.md", "r", encoding="utf-8") as fh:
    long_description = fh.read()

setup(
    name="fman",
    version="1.0.0",
    author="Your Name",
    author_email="your.email@example.com",
    description="A powerful file manager CLI tool",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/yourusername/fman",
    packages=find_packages(),
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Environment :: Console",
        "Topic :: System :: Filesystems",
    ],
    python_requires=">=3.7",
    install_requires=[
        "rich>=10.0.0",
        "tqdm>=4.60.0",
        "click>=8.0.0",
    ],
    entry_points={
        "console_scripts": [
            "fman=fman.cli:main",
        ],
    },
    include_package_data=True,
)
```

## Testing the CLI

```bash
# Install in development mode
pip install -e .

# Test commands
fman list --all
fman search "*.py" --path /home/user/projects
fman copy file.txt backup.txt
fman move old.txt new.txt
fman delete temp.txt --force

# Run tests
pytest tests/ -v
```

## Tips for CLI Development

1. **Clear Command Structure**: Define all commands and options upfront
2. **User Experience**: Request colored output and progress bars
3. **Error Handling**: Specify how errors should be displayed
4. **Configuration**: Include config file support from the start
5. **Testing**: Request unit tests for each command

## Extending the Tool

### Add Compression Support
```markdown
## Additional Command
6. **compress** - Compress files/directories
   - Support zip, tar.gz, tar.bz2
   - Options: --format, --level
   - Show compression ratio
```

### Add Remote Operations
```markdown
## Additional Features
- Support for remote file operations via SSH
- Commands: remote-list, remote-copy, remote-delete
- Use paramiko for SSH connections
```

## Common Patterns

### Confirmation Prompts
```python
def confirm_action(message):
    """Get user confirmation"""
    response = console.input(f"[yellow]{message} [y/N]:[/yellow] ")
    return response.lower() == 'y'
```

### Error Handling
```python
def safe_operation(func):
    """Decorator for safe operations"""
    def wrapper(*args, **kwargs):
        try:
            return func(*args, **kwargs)
        except PermissionError:
            console.print("[red]Permission denied[/red]")
        except FileNotFoundError:
            console.print("[red]File not found[/red]")
        except Exception as e:
            console.print(f"[red]Error: {e}[/red]")
        return False
    return wrapper
```

## Cost Estimation

- **Iterations**: ~30-40 for full implementation
- **Time**: ~15-20 minutes
- **Agent**: Claude or Gemini
- **API Calls**: ~$0.30-0.40