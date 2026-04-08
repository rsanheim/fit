module OutputParser
  OutputRow = Struct.new(:repo, :branch, :message, keyword_init: true)

  def parse_output_line(line)
    parts = line.split("|").map(&:strip)
    return nil unless parts.length == 3

    OutputRow.new(
      repo: parts[0],
      branch: parts[1],
      message: parts[2]
    )
  end

  def parse_output(stdout)
    stdout.lines.filter_map { |line| parse_output_line(line) }
  end

  def find_repo(rows, name)
    rows.find { |r| r.repo == name || r.repo.start_with?(name) }
  end
end
