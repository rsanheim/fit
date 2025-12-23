require "../runner"

module Commands
  class Fetch < Command
    def git_args(repo : String) : Array(String)
      ["fetch"]
    end

    def format_output(stdout : String, stderr : String, success : Bool) : String
      return stderr.each_line.first? || "unknown error" unless success

      # Check if there's any meaningful output
      has_output = stdout.each_line.any? { |l| !l.strip.empty? } ||
                   stderr.each_line.any? { |l| !l.strip.empty? && !l.starts_with?("From") }
      return "no new commits" unless has_output

      # Count branches and tags in single pass
      branch_count = 0
      tag_count = 0
      stdout.each_line do |l|
        next unless l.includes?("->") || l.includes?("[new")
        l.includes?("[new tag]") ? (tag_count += 1) : (branch_count += 1)
      end

      if branch_count > 0 || tag_count > 0
        parts = [] of String
        parts << "#{branch_count} branch#{branch_count == 1 ? "" : "es"}" if branch_count > 0
        parts << "#{tag_count} tag#{tag_count == 1 ? "" : "s"}" if tag_count > 0
        return "#{parts.join(", ")} updated"
      end

      "fetched"
    end
  end
end
