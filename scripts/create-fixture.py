#!/usr/bin/env python3
"""Sanitised sample corpus. These are not Tom's published articles."""
import pathlib,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1]
cli=root/'target/debug/omapress'
path=root/'fixtures/fixing-everything'
if not (path/'publication.toml').exists():subprocess.run([str(cli),'init',str(path),'--name','Fixing Everything — sample','--base-url','https://example.com/publication','--author','Example Author'],check=True,stdout=subprocess.DEVNULL)
for i,(series,title,slug,date,body) in enumerate([
('today-in-omarchy','A place for the work to live','a-place-for-the-work','2026-01-02', 'This is a **sample edition** for testing OmaPress. It is not a published article.\n\n## Keep the source close\n\nWriting begins with an ordinary file. The website, archives and feeds are generated from the same reviewed source.\n\n- Write and edit without a connection.\n- Review your changes before publishing.\n- Keep a permanent address for every story.\n\nRead the [OmaPress repository](https://github.com/tcballard/OmaPress) for the product specification.\n'),
('today-in-omarchy','Small corrections, lasting addresses','small-corrections','2026-01-03','This second **sample edition** exercises feed order and corrected article identity.\n\n## Correct the story\n\nA title can improve while its address remains the same. Feed readers should update the existing item.\n\n```text\nwrite → review → publish → verify\n```\n'),
('unofficial-week-in-omarchy','The week in a few considered notes','sample-weekly-edition','2026-01-04','This is a **sample weekly format**, not a report of real events.\n\n## What changed\n\nUse this section for the week’s verified developments.\n\n## What it means\n\nConnect the changes to the people using the desktop.\n\n## What comes next\n\nKeep claims linked to primary sources.\n')]):
    metadata={'schema':1,'id':f'018fc5c0-2f54-7ea8-aeb2-75e566f3834{i}','title':title,'series':series,'status':'ready','published_at':date+'T21:00:00+00:00','summary':'A sanitised sample article for testing the publication workflow.','slug':slug,'x_caption':'A sample caption. Copy this separately from the article.','tags':['sample','publishing']}
    p=path/'content'/series/(slug+'.md');p.parent.mkdir(parents=True,exist_ok=True)
    p.write_text('---\n'+json.dumps(metadata,indent=2)+'\n---\n'+body)
print(path)
