require "option_parser"
require "./repo"
require "./runner"
require "./commands/*"

VERSION = "0.3.0"
DEFAULT_WORKERS = 8

enum UrlScheme
  SSH
  HTTPS
end

struct Options
  property dry_run : Bool = false
  property workers : Int32 = DEFAULT_WORKERS
  property url_scheme : UrlScheme? = nil
  property command : String? = nil
  property args : Array(String) = [] of String
end

def is_inside_git_repo? : Bool
  result = Process.run("git", ["rev-parse", "--git-dir"], output: Process::Redirect::Close, error: Process::Redirect::Close)
  result.success?
end

def passthrough_to_git(args : Array(String))
  Process.exec("git", args)
end

def print_help
  puts <<-HELP
  fit - parallel git across many repositories

  USAGE:
      fit [OPTIONS] <COMMAND> [ARGS...]

  OPTIONS:
      -n, --workers <NUM>   Number of parallel workers (default: 8, 0=unlimited)
      --dry-run             Print exact commands without executing
      --ssh                 Use SSH URLs
      --https               Use HTTPS URLs
      -h, --help            Print help information
      -V, --version         Print version

  COMMANDS:
      pull      Git pull with condensed output
      fetch     Git fetch with condensed output
      status    Git status with condensed output
      <any>     Pass-through to git verbatim

  EXAMPLES:
      fit pull                      Pull all repos
      fit status                    Status of all repos
      fit --dry-run pull            Show commands without executing
      fit -n 4 fetch                Fetch with 4 workers
      fit checkout main             Switch all repos to main
  HELP
end

def parse_args(argv : Array(String)) : Options
  options = Options.new
  remaining_args = [] of String

  parser = OptionParser.new do |p|
    p.on("-n WORKERS", "--workers=WORKERS", "Number of parallel workers") do |n|
      options.workers = n.to_i
    end

    p.on("--dry-run", "Print commands without executing") do
      options.dry_run = true
    end

    p.on("--ssh", "Use SSH URLs") do
      options.url_scheme = UrlScheme::SSH
    end

    p.on("--https", "Use HTTPS URLs") do
      options.url_scheme = UrlScheme::HTTPS
    end

    p.on("-h", "--help", "Print help") do
      print_help
      exit 0
    end

    p.on("-V", "--version", "Print version") do
      puts "fit #{VERSION}"
      exit 0
    end

    p.unknown_args do |args, _|
      remaining_args = args
    end
  end

  parser.parse(argv)

  if remaining_args.size > 0
    options.command = remaining_args[0]
    options.args = remaining_args[1..]
  end

  options
end

def main
  # Passthrough mode: if inside a git repo, just exec git
  if is_inside_git_repo?
    passthrough_to_git(ARGV)
  end

  options = parse_args(ARGV)

  # Discover repositories
  repos = Repo.discover
  if repos.empty?
    puts "No git repositories found in current directory"
    exit 0
  end

  command = options.command
  if command.nil?
    print_help
    exit 1
  end

  # Run the appropriate command
  case command
  when "status"
    Runner.run(repos, Commands::Status.new, options)
  when "pull"
    Runner.run(repos, Commands::Pull.new, options)
  when "fetch"
    Runner.run(repos, Commands::Fetch.new, options)
  else
    # Passthrough mode for unknown commands
    Runner.run(repos, Commands::Passthrough.new(command, options.args), options)
  end
end

main
