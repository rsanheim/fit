require "../runner"

module Commands
  class Status < Command
    def git_args(repo : String) : Array(String)
      ["status", "--porcelain"]
    end

    def format_output(stdout : String, stderr : String, success : Bool) : String
      return stderr.each_line.first? || "unknown error" unless success

      # Parse porcelain output to count file states
      modified = 0
      added = 0
      deleted = 0
      untracked = 0
      renamed = 0

      stdout.each_line do |line|
        next if line.size < 2

        index_status = line[0]
        worktree_status = line[1]

        # Untracked files
        if index_status == '?'
          untracked += 1
          next
        end

        # Check index status (staged changes)
        case index_status
        when 'M' then modified += 1
        when 'A' then added += 1
        when 'D' then deleted += 1
        when 'R' then renamed += 1
        end

        # Check worktree status (unstaged changes) - only if not already counted
        if index_status == ' '
          case worktree_status
          when 'M' then modified += 1
          when 'D' then deleted += 1
          end
        end
      end

      # Build human-readable summary
      if modified == 0 && added == 0 && deleted == 0 && untracked == 0 && renamed == 0
        return "clean"
      end

      parts = [] of String
      parts << "#{modified} modified" if modified > 0
      parts << "#{added} added" if added > 0
      parts << "#{deleted} deleted" if deleted > 0
      parts << "#{renamed} renamed" if renamed > 0
      parts << "#{untracked} untracked" if untracked > 0

      parts.join(", ")
    end
  end
end
