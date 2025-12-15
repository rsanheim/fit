#!/usr/bin/env ruby
# frozen_string_literal: true

require "optparse"
require "open3"
require "parallel"
require "shellwords"

VERSION = "0.1.0"
MAX_REPO_NAME_WIDTH = 24

# === Passthrough Check ===
# Per SPEC.md 1.1: Must check if inside git repo before any other operation

def inside_git_repo?
  system("git", "rev-parse", "--git-dir", out: File::NULL, err: File::NULL)
end

# === GitCommand Struct ===
# Critical: to_s (dry-run) and run both use to_cmd - same code path per SPEC.md 6.1.1

GitCommand = Struct.new(:repo, :args, :url_scheme) do
  def to_cmd
    cmd = ["git"]
    case url_scheme
    when :ssh
      cmd += ["-c", "url.git@github.com:.insteadOf=https://github.com/"]
    when :https
      cmd += ["-c", "url.https://github.com/.insteadOf=git@github.com:"]
    end
    cmd += ["-C", repo] + args
  end

  def to_s
    to_cmd.shelljoin
  end

  def run
    Open3.capture3({ "GIT_TERMINAL_PROMPT" => "0" }, *to_cmd)
  end
end

# === Output Formatters ===

FORMATTERS = {
  status: lambda { |stdout, stderr, success|
    return "ERROR: #{stderr.lines.first&.strip || 'failed'}" unless success

    modified = 0
    untracked = 0

    stdout.each_line do |line|
      next if line.length < 2

      if line.start_with?("??")
        untracked += 1
      elsif line =~ /^[ MADRCU][MD]|^[MADRCU][ MD]/
        modified += 1
      end
    end

    return "clean" if modified.zero? && untracked.zero?

    parts = []
    parts << "#{modified} modified" if modified > 0
    parts << "#{untracked} untracked" if untracked > 0
    parts.join(", ")
  },

  pull: lambda { |stdout, stderr, success|
    return "ERROR: #{stderr.lines.first&.strip || 'failed'}" unless success
    return "Already up to date" if stdout.include?("Already up to date")

    # Match both "1 file changed" and "N files changed"
    stdout.lines.find { |l| l.include?("file changed") || l.include?("files changed") }&.strip || "completed"
  },

  fetch: lambda { |stdout, stderr, success|
    return "ERROR: #{stderr.lines.first&.strip || 'failed'}" unless success
    return "no new commits" if stdout.strip.empty? && stderr.strip.empty?

    "fetched"
  },

  passthrough: lambda { |stdout, stderr, success|
    return "ERROR: #{stderr.lines.first&.strip || 'failed'}" unless success

    stdout.lines.first&.strip || stderr.lines.first&.strip || "ok"
  }
}.freeze

# === Helper Functions ===

def find_repos
  Dir.glob("*/.git").map { |p| File.dirname(p) }.sort
end

def format_repo_name(name)
  display = if name.length > MAX_REPO_NAME_WIDTH
    "#{name[0, MAX_REPO_NAME_WIDTH - 4]}-..."
  else
    name
  end
  "[#{display.ljust(MAX_REPO_NAME_WIDTH)}]"
end

# === Parallel Execution ===

def run_parallel(repos, command:, args:, dry_run:, workers:, url_scheme:, formatter:)
  # Build commands for all repos
  commands = repos.map do |repo|
    full_args = case command
    when "status"
      ["status", "--porcelain"] + args
    else
      [command] + args
    end
    GitCommand.new(repo, full_args, url_scheme)
  end

  # Dry-run: print commands and exit
  if dry_run
    puts "[nit v#{VERSION}] Running in **dry-run mode**, no git commands will be executed. Planned git commands below."
    commands.each { |cmd| puts cmd.to_s }
    return
  end

  # Parallel execution with processes
  worker_count = workers.zero? ? repos.size : [workers, repos.size].min

  results = Parallel.map(commands, in_processes: worker_count) do |cmd|
    stdout, stderr, status = cmd.run
    { repo: cmd.repo, stdout: stdout, stderr: stderr, success: status.success? }
  end

  # Print results in order (Parallel.map preserves order)
  results.each do |r|
    name = format_repo_name(File.basename(r[:repo]))
    output = formatter.call(r[:stdout], r[:stderr], r[:success])
    puts "#{name} #{output}"
  end
end

# === Main Entry Point ===
# Only execute when run directly (not when required for testing)

if __FILE__ == $0
  # Passthrough check FIRST per SPEC.md 1.1
  if inside_git_repo?
    exec("git", *ARGV) # Never returns - replaces process with git
  end

  # CLI Parsing
  options = { dry_run: false, workers: 8, url_scheme: nil }

  parser = OptionParser.new do |opts|
    opts.banner = "Usage: nit [OPTIONS] <COMMAND> [ARGS...]"
    opts.separator ""
    opts.separator "Commands:"
    opts.separator "  status    Show status of all repositories"
    opts.separator "  pull      Pull all repositories"
    opts.separator "  fetch     Fetch all repositories"
    opts.separator "  <other>   Pass through to git"
    opts.separator ""
    opts.separator "Options:"

    opts.on("--dry-run", "Print exact commands without executing") do
      options[:dry_run] = true
    end

    opts.on("-n", "--max-connections NUM", Integer, "Max concurrent git processes (default: 8, 0 = unlimited)") do |n|
      options[:workers] = n
    end

    opts.on("--ssh", "Force SSH URLs for all remotes") do
      options[:url_scheme] = :ssh
    end

    opts.on("--https", "Force HTTPS URLs for all remotes") do
      options[:url_scheme] = :https
    end

    opts.on("-V", "--version", "Print version") do
      puts "nit #{VERSION}"
      exit
    end

    opts.on("-h", "--help", "Print help") do
      puts opts
      exit
    end
  end

  begin
    args = parser.order(ARGV) # Parse options, stop at first non-option
  rescue OptionParser::InvalidOption => e
    warn "nit: #{e.message}"
    exit 1
  end

  command = args.shift
  git_args = args

  # Main Execution
  repos = find_repos

  if repos.empty?
    puts "No git repositories found in current directory"
    exit 0
  end

  # Handle no command - default to showing help
  if command.nil?
    warn "nit: no command specified"
    warn parser
    exit 1
  end

  formatter = case command
  when "status" then FORMATTERS[:status]
  when "pull" then FORMATTERS[:pull]
  when "fetch" then FORMATTERS[:fetch]
  else FORMATTERS[:passthrough]
  end

  run_parallel(
    repos,
    command: command,
    args: git_args,
    dry_run: options[:dry_run],
    workers: options[:workers],
    url_scheme: options[:url_scheme],
    formatter: formatter
  )
end
