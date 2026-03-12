#!/usr/bin/env python3
import argparse
import asyncio
import csv
import re

import asyncpg


INT32_MAX = 2_147_483_647


def parse_product_id(code, fallback_id):
    nums = re.sub(r'[^0-9]', '', code)
    if nums:
        val = int(nums)
        if val <= INT32_MAX:
            return val
    fallback = fallback_id.strip()
    if fallback:
        return int(fallback)
    return None


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--db-url", required=True)
    parser.add_argument("--products-csv", default="tests/news.csv")
    parser.add_argument("--source-category", type=int, default=12)
    parser.add_argument("--target-category", type=int, default=4)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()

    csv_products = []
    with open(args.products_csv, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            if row.get('category', '').strip() != str(args.source_category):
                continue
            code = row.get('code', '').strip()
            fallback_id = row.get('id', '').strip()
            pid = parse_product_id(code, fallback_id)
            if pid is None:
                continue
            csv_products.append({
                'csv_id': fallback_id,
                'code': code,
                'title': row.get('title', '').strip()[:80],
                'product_id': pid,
                'used_fallback': int(re.sub(r'[^0-9]', '', code) or '0') > INT32_MAX,
            })

    print(f"Found {len(csv_products)} products with category={args.source_category} in CSV")

    if not csv_products:
        print("Nothing to update.")
        return

    if args.limit > 0:
        csv_products = csv_products[:args.limit]
        print(f"Limited to {args.limit} product(s)")

    print("\n--- CSV data ---")
    for p in csv_products:
        id_source = f"fallback id={p['csv_id']}" if p['used_fallback'] else f"code={p['code']}"
        print(f"  CSV row: id={p['csv_id']}, code={p['code']}, title={p['title']}")
        print(f"    -> product_id={p['product_id']} (from {id_source})")

    product_ids = [p['product_id'] for p in csv_products]

    pool = await asyncpg.create_pool(args.db_url)
    try:
        async with pool.acquire() as conn:
            existing = await conn.fetch(
                """SELECT p.id, p.name, pc.category_id
                   FROM products p
                   LEFT JOIN product_categories pc ON pc.product_id = p.id
                   WHERE p.id = ANY($1::int[])""",
                product_ids
            )

            if not existing:
                print("\nNo matching products found in DB.")
                return

            print(f"\n--- DB matches ({len(existing)}) ---")
            for r in existing:
                print(f"  id={r['id']}, name={r['name'][:80]}, current_category={r['category_id']}")

            existing_ids = list(set(r['id'] for r in existing))

            if args.dry_run:
                print(f"\n[DRY RUN] Would update {len(existing_ids)} products: category {args.source_category} -> {args.target_category}")
                return

            await conn.executemany(
                """INSERT INTO product_categories (product_id, category_id)
                   VALUES ($1, $2)
                   ON CONFLICT (product_id, category_id) DO NOTHING""",
                [(pid, args.target_category) for pid in existing_ids]
            )

            removed = await conn.execute(
                """DELETE FROM product_categories
                   WHERE product_id = ANY($1::int[]) AND category_id = $2""",
                existing_ids, args.source_category
            )

            updated = await conn.fetch(
                """SELECT p.id, p.name, pc.category_id
                   FROM products p
                   JOIN product_categories pc ON pc.product_id = p.id
                   WHERE p.id = ANY($1::int[])""",
                existing_ids
            )
            print(f"\n--- After update ---")
            for r in updated:
                print(f"  id={r['id']}, name={r['name'][:80]}, category={r['category_id']}")
            print(f"Removed old mappings: {removed}")

        print("Done!")
    finally:
        await pool.close()


if __name__ == "__main__":
    asyncio.run(main())
