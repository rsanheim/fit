require "option_parser"
require "./version"
require "./repo"
require "./runner"
require "./commands/*"
require "./meta"

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
      Meta.help
      exit 0
    end

    p.on("-V", "--version", "Print version") do
      puts "fit #{Fit::VERSION}"
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
  if Meta.dispatch(ARGV)
    exit 0
  end

  if is_inside_git_repo?
    passthrough_to_git(ARGV)
  end

  options = parse_args(ARGV)

  repos = Repo.discover
  if repos.empty?
    puts "No git repositories found in current directory"
    exit 0
  end

  command = options.command
  if command.nil?
    Meta.help
    exit 1
  end

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
