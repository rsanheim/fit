require "../runner"

module Commands
  class Passthrough < Command
    property command : String
    property extra_args : Array(String)

    def initialize(@command, @extra_args = [] of String)
    end

    def git_args(repo : String) : Array(String)
      args = [@command]
      args.concat(@extra_args)
      args
    end

    def format_output(stdout : String, stderr : String, success : Bool) : String
      unless success
        error_line = stderr.each_line.find { |l| !l.strip.empty? } || "unknown error"
        return "ERROR: #{error_line}"
      end

      # For passthrough, just show first non-empty line or "ok"
      stdout.each_line do |line|
        return line.strip unless line.strip.empty?
      end
      stderr.each_line do |line|
        return line.strip unless line.strip.empty?
      end

      "ok"
    end
  end
end
