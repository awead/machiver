#! /usr/local/bin/ruby

require "date"
require "exif"
require "forwardable"
require "json"
require "optparse"
require "pathname"
require "pry"
require "securerandom"

def options
  @options ||= OpenStruct.new
end

ARGV.options do |opts|
  opts.on("-a", "--archive=", String) { |val| options.archive = val }
  opts.on("-x", "--extension=", String) { |val| options.extension = val }
  opts.on("-R", "--recursive") { |val| options.recursive = true }
  opts.on("-i", "--info") { |val| options.info = true }

  opts.on_tail("-h", "--help") do
    puts opts
    exit
  end

  opts.parse!
end

class ArchiveFile
  attr_reader :file, :dest

  def initialize(file:, dest:)
    @file = Pathname.new(file)
    @dest = dest
  end

  def data
    @data ||= begin
      Exif::Data.new(file.open)
    rescue Exif::NotReadable
      NullData.new
    end
  end

  def analyze
    puts "#{file} -- #{pp data.to_h}"
  end

  def extname
    file.extname.downcase
  end

  ##
  # .birthtime will raise a NotImplementedError in some filesystems, notably with Docker, so we'll default to
  # modification time which seems to be the best fit for files exported from their original locations.
  def original_date
    @original_date ||= begin
      DateTime.parse(date_time_original)
    rescue NotImplementedError
      file.mtime
    end
  end

  def move
    FileUtils.mkdir_p(new_path)
    FileUtils.mv(file, new_path.join(SecureRandom.uuid + extname))
  end

  private

  def date_time_original
    date, _ = data.date_time_original&.split
    return file.birthtime if date.nil?

    if date.match?(/\d\d\.\d\d\.\d\d\d\d/)
      date.split(".").rotate(-1).join("-")
    else
      date.tr(":", "-")
    end
  end

  def new_path
    @new_path ||= dest
      .join(original_date.strftime("%Y"))
      .join(original_date.strftime("%m"))
      .join(original_date.strftime("%d"))
  end

  class NullData
    def date_time_original
      ""
    end

    def to_h
      {}
    end
  end
end

def call(file)
  if options.info
    file.analyze
  elsif file.extname.match?(options.extension || "jpg")
    file.move
  end
end

def destination
  if options.archive
    Pathname.new(options.archive)
  else
    Pathname.pwd
  end
end

def recursive(dir)
  dir.each_child do |path|
    if path.directory? && options.recursive
      recursive(path)
    else
      call(ArchiveFile.new(file: path, dest: destination))
    end
  end
end

recursive(Pathname.getwd)
