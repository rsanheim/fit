require "./repo"

# Base class for all commands
abstract class Command
  # Build git arguments for this command
  abstract def git_args(repo : String) : Array(String)

  # Format the output from git into a single line
  abstract def format_output(stdout : String, stderr : String, success : Bool) : String
end

# A running git process with captured output
struct RunningProcess
  property repo : String
  property process : Process
  property stdout : IO::Memory
  property stderr : IO::Memory
  property index : Int32

  def initialize(@repo, @process, @stdout, @stderr, @index)
  end
end

# A completed process result
struct CompletedResult
  property repo : String
  property stdout : String
  property stderr : String
  property success : Bool
  property index : Int32

  def initialize(@repo, @stdout, @stderr, @success, @index)
  end
end

module Runner
  def self.run(repos : Array(String), command : Command, options : Options)
    url_scheme = options.url_scheme

    # Handle dry-run mode
    if options.dry_run
      repos.each do |repo|
        args = build_git_args(repo, command.git_args(repo), url_scheme)
        puts "git #{args.join(" ")}"
      end
      return
    end

    workers = options.workers
    # Use unlimited when workers=0 or workers >= repo count
    if workers == 0 || workers >= repos.size
      run_unlimited(repos, command, url_scheme)
    else
      run_limited(repos, command, url_scheme, workers)
    end
  end

  # Build the full git argument list including -C and scheme overrides
  private def self.build_git_args(repo : String, cmd_args : Array(String), url_scheme : UrlScheme?) : Array(String)
    args = [] of String

    # URL scheme override (must come before other args)
    case url_scheme
    when UrlScheme::SSH
      args << "-c" << "url.git@github.com:.insteadOf=https://github.com/"
    when UrlScheme::HTTPS
      args << "-c" << "url.https://github.com/.insteadOf=git@github.com:"
    end

    args << "-C" << repo
    args.concat(cmd_args)
    args
  end

  # Spawn a git process for a repo
  private def self.spawn_process(repo : String, command : Command, url_scheme : UrlScheme?, index : Int32) : RunningProcess
    args = build_git_args(repo, command.git_args(repo), url_scheme)
    stdout = IO::Memory.new
    stderr = IO::Memory.new

    process = Process.new(
      "git",
      args,
      output: stdout,
      error: stderr,
      env: {"GIT_TERMINAL_PROMPT" => "0"}
    )

    RunningProcess.new(repo, process, stdout, stderr, index)
  end

  # Unlimited mode: spawn all processes immediately, wait in order
  private def self.run_unlimited(repos : Array(String), command : Command, url_scheme : UrlScheme?)
    # Phase 1: Spawn all processes immediately
    processes = repos.map_with_index do |repo, idx|
      spawn_process(repo, command, url_scheme, idx)
    end

    # Phase 2: Wait and print in order
    processes.each do |proc|
      status = proc.process.wait
      stdout = proc.stdout.to_s
      stderr = proc.stderr.to_s
      output = command.format_output(stdout, stderr, status.success?)
      print_result(proc.repo, output)
    end
  end

  # Limited mode: sliding window with at most N active processes
  private def self.run_limited(repos : Array(String), command : Command, url_scheme : UrlScheme?, max_workers : Int32)
    next_to_spawn = 0
    next_to_print = 0
    active = [] of RunningProcess
    completed = [] of CompletedResult

    # Initial burst: spawn up to max_workers
    while next_to_spawn < repos.size && active.size < max_workers
      active << spawn_process(repos[next_to_spawn], command, url_scheme, next_to_spawn)
      next_to_spawn += 1
    end

    # Main loop: poll active processes, spawn new ones, print completed in order
    while !active.empty? || next_to_print < repos.size
      # Check each active process
      i = 0
      while i < active.size
        proc = active[i]
        if !proc.process.exists?
          # Process finished - remove from active
          active.delete_at(i)
          status = proc.process.wait
          stdout = proc.stdout.to_s
          stderr = proc.stderr.to_s
          completed << CompletedResult.new(proc.repo, stdout, stderr, status.success?, proc.index)
          # Don't increment i - we removed an element
        else
          i += 1
        end
      end

      # Spawn new processes if we have capacity
      while next_to_spawn < repos.size && active.size < max_workers
        active << spawn_process(repos[next_to_spawn], command, url_scheme, next_to_spawn)
        next_to_spawn += 1
      end

      # Print any completed outputs that are ready (in order)
      loop do
        idx = completed.index { |c| c.index == next_to_print }
        break unless idx

        result = completed.delete_at(idx)
        output = command.format_output(result.stdout, result.stderr, result.success)
        print_result(result.repo, output)
        next_to_print += 1
      end

      # If all printed, we're done
      break if next_to_print >= repos.size

      # Small sleep to avoid busy-waiting
      sleep 5.milliseconds unless active.empty?
    end
  end

  private def self.print_result(repo : String, output : String)
    formatted_name = Repo.format_name(repo)
    puts "[#{formatted_name}] #{output}"
  end
end
