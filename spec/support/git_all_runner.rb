require "open3"

module GitAllRunner
  RunResult = Struct.new(:stdout, :stderr, :status, :exit_code, keyword_init: true)

  def git_all_bin
    ENV.fetch("GIT_ALL_BIN") {
      File.expand_path("../../bin/git-all-rust", __dir__)
    }
  end

  def run_git_all(*args, dir: @workspace)
    stdout, stderr, status = Open3.capture3(git_all_bin, *args, chdir: dir)
    RunResult.new(stdout: stdout, stderr: stderr, status: status, exit_code: status.exitstatus)
  end
end
