require "../runner"

module Commands
  class Pull < Command
    def git_args(repo : String) : Array(String)
      ["pull"]
    end

    def format_output(stdout : String, stderr : String, success : Bool) : String
      return stderr.each_line.first? || "unknown error" unless success

      # Check for "Already up to date"
      if stdout.includes?("Already up to date")
        return "Already up to date"
      end

      # Try to extract summary from stdout (e.g., "3 files changed, 10 insertions(+), 5 deletions(-)")
      stdout.each_line do |line|
        if line.includes?("files changed")
          return line.strip
        end
      end

      # Check for fast-forward or merge info in stdout
      stdout.each_line do |line|
        if line.includes?("..") || line.includes?("Updating")
          return line.strip
        end
      end

      # Fallback: first non-empty line of stdout, or stderr
      stdout.each_line do |line|
        return line.strip unless line.strip.empty?
      end
      stderr.each_line do |line|
        return line.strip unless line.strip.empty?
      end

      "completed"
    end
  end
end
