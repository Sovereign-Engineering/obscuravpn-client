using System;
using System.Text;
using System.Text.RegularExpressions;
using Markdig;
using Markdig.Extensions.EmphasisExtras;
using Markdig.Syntax;
using Markdig.Syntax.Inlines;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Windows.UI.Text;

namespace Obscura_Client.Markdown;

/// <summary>
/// Renders Markdown by mapping Markdig parsed syntax tree onto XAML text element
/// </summary>
static partial class MarkdownText
{
    const double Indent = 8;
    const int MaxDepth = 32;

    static readonly MarkdownPipeline Pipeline = new MarkdownPipelineBuilder()
        .DisableHtml()
        .UseEmphasisExtras(EmphasisExtraOptions.Strikethrough)
        .Build();

    static readonly FontFamily CodeFont = new("Cascadia Mono, Consolas, Courier New");

    // bidirectional overrides/isolates can visually reorder text, e.g. to disguise a link label.
    [GeneratedRegex(@"[\u202A-\u202E\u2066-\u2069]")]
    private static partial Regex BidiControls();

    public static void Render(RichTextBlock target, string? markdown)
    {
        target.Blocks.Clear();
        if (string.IsNullOrWhiteSpace(markdown))
        {
            return;
        }
        foreach (var block in Markdig.Markdown.Parse(markdown, Pipeline))
        {
            AddBlock(target.Blocks, block, indent: 0, depth: 0);
        }
    }

    static void AddBlock(BlockCollection blocks, Markdig.Syntax.Block block, double indent, int depth)
    {
        if (depth > MaxDepth)
        {
            return;
        }
        switch (block)
        {
            case HeadingBlock heading:
            {
                var p = NewParagraph(blocks, indent, spacing: 12);
                p.FontSize = heading.Level switch { 1 => 20, 2 => 16, _ => 14 };
                p.FontWeight = FontWeights.SemiBold;
                AddInlines(p.Inlines, heading.Inline);
                blocks.Add(p);
                break;
            }
            case ParagraphBlock paragraph:
            {
                var p = NewParagraph(blocks, indent, spacing: 8);
                AddInlines(p.Inlines, paragraph.Inline);
                blocks.Add(p);
                break;
            }
            case ListBlock list:
                foreach (var child in list)
                {
                    if (child is ListItemBlock item)
                    {
                        AddListItem(blocks, item, list.IsOrdered ? $"{item.Order}." : "\u2022", indent, depth + 1);
                    }
                }
                break;
            case QuoteBlock quote:
            {
                var first = blocks.Count;
                foreach (var child in quote)
                {
                    AddBlock(blocks, child, indent + Indent, depth + 1);
                }
                for (var i = first; i < blocks.Count; i++)
                {
                    blocks[i].Foreground = SecondaryBrush();
                    blocks[i].FontStyle = FontStyle.Italic;
                }
                break;
            }
            case CodeBlock code:
            {
                var p = NewParagraph(blocks, indent, spacing: 8);
                p.FontFamily = CodeFont;
                p.Inlines.Add(new Run { Text = Clean(code.Lines.ToString().TrimEnd('\n')) });
                blocks.Add(p);
                break;
            }
            case ThematicBreakBlock:
            {
                var p = NewParagraph(blocks, indent, spacing: 8);
                p.Inlines.Add(new Run { Text = new string('\u2500', 24), Foreground = SecondaryBrush() });
                blocks.Add(p);
                break;
            }
        }
    }

    static void AddListItem(BlockCollection blocks, ListItemBlock item, string marker, double indent, int depth)
    {
        var p = NewParagraph(blocks, indent + Indent, spacing: 4);
        p.Inlines.Add(new Run { Text = marker + "\u00A0\u00A0" });
        var start = 0;
        if (item.Count > 0 && item[0] is ParagraphBlock text)
        {
            AddInlines(p.Inlines, text.Inline);
            start = 1;
        }
        blocks.Add(p);
        for (var i = start; i < item.Count; i++)
        {
            AddBlock(blocks, item[i], indent + Indent, depth + 1);
        }
    }

    static Paragraph NewParagraph(BlockCollection blocks, double indent, double spacing) =>
        new() { Margin = new Thickness(indent, blocks.Count == 0 ? 0 : spacing, 0, 0) };

    static void AddInlines(InlineCollection target, ContainerInline? container, int depth = 0)
    {
        if (container == null)
        {
            return;
        }
        if (depth > MaxDepth)
        {
            target.Add(new Run { Text = PlainText(container) });
            return;
        }
        foreach (var inline in container)
        {
            switch (inline)
            {
                case LiteralInline literal:
                    target.Add(new Run { Text = Clean(literal.Content.ToString()) });
                    break;
                case HtmlEntityInline entity:
                    target.Add(new Run { Text = Clean(entity.Transcoded.ToString()) });
                    break;
                case CodeInline code:
                    target.Add(new Run { Text = Clean(code.Content), FontFamily = CodeFont });
                    break;
                case LineBreakInline lineBreak:
                    target.Add(lineBreak.IsHard ? new LineBreak() : new Run { Text = " " });
                    break;
                case EmphasisInline { DelimiterChar: '~' } strike:
                {
                    var span = new Span { TextDecorations = TextDecorations.Strikethrough };
                    AddInlines(span.Inlines, strike, depth + 1);
                    target.Add(span);
                    break;
                }
                case EmphasisInline emphasis:
                {
                    Span span = emphasis.DelimiterCount >= 2 ? new Bold() : new Italic();
                    AddInlines(span.Inlines, emphasis, depth + 1);
                    target.Add(span);
                    break;
                }
                case LinkInline { IsImage: true } image:
                    target.Add(new Run { Text = PlainText(image), Foreground = SecondaryBrush() });
                    break;
                case LinkInline link:
                    AddLink(target, link.Url, PlainText(link));
                    break;
                case AutolinkInline autolink:
                    AddLink(target, autolink.Url, autolink.Url);
                    break;
                case ContainerInline other:
                    AddInlines(target, other, depth + 1);
                    break;
            }
        }
    }

    /// <summary>
    /// Link labels that contain formatting e.g. <c>[**bold** text](url)</c>
    /// are rendered as plain text instead of nesting styled spans inside the Hyperlink.
    /// </summary>
    /// <param name="target">Inlines of the paragraph the link is appended to.</param>
    /// <param name="url">Link destination as written in the Markdown; only http(s) becomes clickable,
    /// anything else (or an unparseable URL) is rendered as the label text alone.</param>
    /// <param name="label">Plain-text link label; falls back to the URL when empty.</param>
    static void AddLink(InlineCollection target, string? url, string label)
    {
        label = Clean(string.IsNullOrEmpty(label) ? url ?? "" : label);
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) || uri.Scheme is not ("http" or "https"))
        {
            target.Add(new Run { Text = label });
            return;
        }
        var hyperlink = new Hyperlink { NavigateUri = uri };
        hyperlink.Inlines.Add(new Run { Text = label });
        ToolTipService.SetToolTip(hyperlink, uri.AbsoluteUri);
        target.Add(hyperlink);
    }

    static string PlainText(ContainerInline container)
    {
        var sb = new StringBuilder();
        foreach (var inline in container.Descendants())
        {
            switch (inline)
            {
                case LiteralInline literal:
                    sb.Append(literal.Content.AsSpan());
                    break;
                case HtmlEntityInline entity:
                    sb.Append(entity.Transcoded.AsSpan());
                    break;
                case CodeInline code:
                    sb.Append(code.Content);
                    break;
            }
        }
        return Clean(sb.ToString());
    }

    static string Clean(string text) => BidiControls().Replace(text, "");

    static Brush SecondaryBrush() =>
        Application.Current.Resources.TryGetValue("TextFillColorSecondaryBrush", out var brush) && brush is Brush b
            ? b
            : new SolidColorBrush(Microsoft.UI.Colors.Gray);
}
